//! MySQL 8 reads use guarded SELECTs and one reusable, read-only transport per workspace.
use crate::{
    models::*,
    policy::{self, MAX_RESULT_BYTES, MAX_ROWS},
    remote_profile::RemoteProfile,
};
use mysql_async::{Conn, OptsBuilder, Params, Row, SslOpts, Value, prelude::Queryable};
use std::time::{Duration, Instant};

pub fn quote(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}
fn db_error(error: mysql_async::Error) -> AppError {
    use mysql_async::{DriverError, Error, IoError};
    let (code, message) = match error {
        Error::Io(IoError::Tls(_)) => (
            "MYSQL_TLS",
            "MySQL TLS verification failed. Use the certificate's hostname and its CA PEM file. For a trusted endpoint with a self-signed certificate, explicitly choose 'TLS · encrypted, certificate not verified' in Transport security. Plaintext fallback is disabled.",
        ),
        Error::Driver(DriverError::NoClientSslFlagFromServer) => (
            "MYSQL_TLS",
            "This MySQL endpoint does not offer TLS. Enable TLS on the server, or use Local only with a trusted loopback tunnel.",
        ),
        Error::Io(IoError::Io(ref e)) if e.kind() == std::io::ErrorKind::TimedOut => (
            "TIMEOUT",
            "The MySQL endpoint did not respond in time. Check the host, port, firewall and network.",
        ),
        Error::Io(_) | Error::Driver(DriverError::ConnectionClosed) => (
            "MYSQL_CONNECTION",
            "Cannot communicate with the MySQL endpoint. Check the host, port, firewall and whether the server is running.",
        ),
        Error::Driver(
            DriverError::UnknownAuthPlugin { .. }
            | DriverError::MysqlOldPasswordDisabled
            | DriverError::CleartextPluginDisabled,
        ) => (
            "MYSQL_AUTH_PLUGIN",
            "This account uses an unsupported or disabled authentication plugin. Ask the administrator for a supported MySQL 8 account; insecure authentication is not enabled.",
        ),
        Error::Server(ref e) => match e.code {
            1045 | 1698 | 3118 => (
                "MYSQL_AUTH",
                "MySQL rejected authentication. Check the username, password, account lock status and allowed client host.",
            ),
            1044 | 1142 | 1143 | 1227 => (
                "MYSQL_PERMISSION",
                "The account lacks permission for this database or operation. Use direct SELECT / SHOW VIEW grants on the selected database; ask the administrator to verify access.",
            ),
            1049 => (
                "MYSQL_DATABASE",
                "The selected MySQL database does not exist. Check the database name, including letter case.",
            ),
            1040 | 1203 => (
                "MYSQL_CONNECTION_LIMIT",
                "MySQL has reached its connection limit. Close unused sessions or ask the administrator to check the server limit.",
            ),
            1129 | 1130 => (
                "MYSQL_HOST",
                "MySQL does not allow this client host. Ask the administrator to check the account host and blocked-host settings.",
            ),
            1193 | 1231 => (
                "MYSQL_UNSUPPORTED",
                "This endpoint does not support the required MySQL session settings. Use a compatible MySQL 8 server; MariaDB and emulated endpoints are not yet supported.",
            ),
            1205 | 3024 => (
                "TIMEOUT",
                "MySQL stopped this operation at the query or lock timeout. Narrow the filters or inspect the query plan.",
            ),
            1317 => ("CANCELLED", "The MySQL operation was cancelled."),
            3159 => (
                "MYSQL_TLS",
                "MySQL requires an encrypted connection. Select verified TLS and provide the server's CA certificate if needed.",
            ),
            1146 => (
                "MYSQL_SCHEMA_CHANGED",
                "The table no longer exists or is not visible. Reconnect to refresh the MySQL catalog.",
            ),
            _ => (
                "MYSQL_QUERY",
                "MySQL rejected this statement. Check column types and values, then refresh the schema. Raw server messages are withheld.",
            ),
        },
        Error::Driver(DriverError::PacketTooLarge) => (
            "RESULT_LIMIT",
            "The MySQL response exceeds the result byte limit. Select fewer or smaller columns.",
        ),
        _ => (
            "MYSQL_PROTOCOL",
            "The MySQL driver could not process this response. Check server compatibility and supported column types.",
        ),
    };
    AppError::new(code, message)
}

// mysql_async normally drains pending results in a background task on Drop.
// Own the connection so cancellation closes it instead of draining a large read.
pub struct Database {
    conn: Option<Conn>,
    version: String,
    reused: bool,
    reusable: bool,
    idle_since: Instant,
    connection_ms: u64,
}
impl std::ops::Deref for Database {
    type Target = Conn;
    fn deref(&self) -> &Conn {
        self.conn.as_ref().expect("live connection")
    }
}
impl std::ops::DerefMut for Database {
    fn deref_mut(&mut self) -> &mut Conn {
        self.conn.as_mut().expect("live connection")
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(1), conn.disconnect())
                    .await;
            });
        }
    }
}

pub async fn open(p: &RemoteProfile) -> AppResult<Database> {
    let started = Instant::now();
    let idle = p.mysql_idle.lock().ok().and_then(|mut slot| slot.take());
    if let Some(mut db) = idle
        && !db.is_disconnected()
    {
        // No extra network round trip on an active workspace. After an idle
        // period, detect server/network expiry before sending an application read.
        if db.idle_since.elapsed() < Duration::from_secs(30) || db.ping().await.is_ok() {
            db.reused = true;
            db.connection_ms = started.elapsed().as_millis() as u64;
            return Ok(db);
        }
    }
    let mut db = open_dedicated(p).await?;
    db.connection_ms = started.elapsed().as_millis() as u64;
    Ok(db)
}

fn recycle(p: &RemoteProfile, mut db: Database) {
    // Only call after successful, fully consumed, session-neutral operations.
    // Errors, timeout/cancellation and incomplete reads drop/close the transport.
    if db.reusable
        && !db.is_disconnected()
        && let Ok(mut slot) = p.mysql_idle.lock()
    {
        db.idle_since = Instant::now();
        *slot = Some(db);
    }
}

async fn open_dedicated(p: &RemoteProfile) -> AppResult<Database> {
    let started = Instant::now();
    let tls = SslOpts::default()
        .with_danger_accept_invalid_certs(p.tls == TlsMode::Require)
        .with_danger_skip_domain_validation(p.tls == TlsMode::Require)
        .with_root_certs(if p.root_ca_pem.is_empty() {
            vec![]
        } else {
            vec![p.root_ca_pem.as_bytes().to_vec().into()]
        });
    let opts = OptsBuilder::default()
        .ip_or_hostname(&p.host)
        .tcp_port(p.endpoint.map_or(p.port, |e| e.port()))
        .resolved_ips(p.endpoint.map(|e| vec![e.ip()]))
        .user(Some(p.username.clone()))
        .pass(Some(p.password.to_string()))
        .db_name((!p.database.is_empty()).then(|| p.database.clone()))
        .prefer_socket(false)
        .enable_cleartext_plugin(false)
        .stmt_cache_size(32)
        .max_allowed_packet(Some(MAX_RESULT_BYTES))
        .ssl_opts((p.tls != TlsMode::LocalOnly).then_some(tls));
    let raw = Conn::new(opts).await.map_err(|error| {
        if p.tls == TlsMode::Require && matches!(error, mysql_async::Error::Io(mysql_async::IoError::Tls(_))) {
            AppError::new("MYSQL_TLS", "MySQL TLS handshake failed. Check the server TLS configuration and whether a client certificate is required. Plaintext fallback is disabled.")
        } else { db_error(error) }
    })?;
    let mut conn = Database {
        conn: Some(raw),
        version: String::new(),
        reused: false,
        reusable: true,
        idle_since: Instant::now(),
        connection_ms: 0,
    };
    let version: String = conn
        .query_first("SELECT VERSION()")
        .await
        .map_err(db_error)?
        .unwrap_or_default();
    if !version.starts_with("8.") || version.contains("MariaDB") {
        return Err(AppError::policy(
            "This adapter supports MySQL 8.x. MariaDB has not been validated.",
        ));
    }
    conn.version = version;
    // Account privileges do not enable writes in the application. Every operation
    // uses fixed metadata SQL or a reconstructed SELECT. Session READ ONLY
    // covers every implicit transaction; autocommit prevents idle snapshots,
    // metadata locks and stale reads between operations on a retained socket.
    conn.query_drop("SET SESSION sql_mode = 'STRICT_TRANS_TABLES,NO_BACKSLASH_ESCAPES', max_execution_time = 10000, lock_wait_timeout = 2, transaction_read_only = 1, autocommit = 1").await.map_err(db_error)?;
    conn.connection_ms = started.elapsed().as_millis() as u64;
    Ok(conn)
}

fn selected_database<'a>(p: &'a RemoteProfile, schema: Option<&'a str>) -> AppResult<&'a str> {
    let name = schema.filter(|s| !s.is_empty()).unwrap_or(&p.database);
    if name.is_empty() {
        return Err(AppError::new(
            "MYSQL_DATABASE_REQUIRED",
            "Expand a database and select a table first, or use a database-qualified table in SQL.",
        ));
    }
    if name.len() > 253 || name.contains('\0') {
        return Err(AppError::policy("Invalid database name."));
    }
    Ok(name)
}

pub async fn schemas(p: &RemoteProfile) -> AppResult<Vec<String>> {
    let mut conn = open(p).await?;
    let names: Vec<String> = conn
        .query(
            "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME LIMIT 1001",
        )
        .await
        .map_err(db_error)?;
    if names.len() > 1000 {
        return Err(AppError::policy(
            "This server exposes more than 1000 databases.",
        ));
    }
    recycle(p, conn);
    Ok(names)
}

pub async fn schema_tables(p: &RemoteProfile, schema: &str) -> AppResult<Vec<TableInfo>> {
    let schema = selected_database(p, Some(schema))?;
    let mut conn = open(p).await?;
    let tables = checked_catalog(p, &mut conn, schema, None).await?;
    recycle(p, conn);
    Ok(tables)
}

pub async fn tables(conn: &mut Conn, schema: &str) -> AppResult<Vec<TableInfo>> {
    catalog(conn, schema, None).await
}

async fn catalog(conn: &mut Conn, schema: &str, table: Option<&str>) -> AppResult<Vec<TableInfo>> {
    let records: Vec<Row> = conn.exec("SELECT t.TABLE_NAME, t.TABLE_ROWS, c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE, c.COLUMN_KEY, c.EXTRA, t.TABLE_TYPE, t.ENGINE FROM information_schema.TABLES t LEFT JOIN information_schema.COLUMNS c ON c.TABLE_SCHEMA=t.TABLE_SCHEMA AND c.TABLE_NAME=t.TABLE_NAME WHERE t.TABLE_SCHEMA=? AND (? IS NULL OR t.TABLE_NAME=?) ORDER BY t.TABLE_NAME,c.ORDINAL_POSITION LIMIT 16385", (schema, table, table)).await.map_err(db_error)?;
    if records.len() > 16384 {
        return Err(AppError::policy(
            "MySQL catalog exceeds the 16,384-column metadata limit.",
        ));
    }
    let mut tables: Vec<TableInfo> = Vec::new();
    for row in records {
        let name: String = row.get(0).unwrap_or_default();
        let ty = row
            .get::<Option<String>, _>(3)
            .flatten()
            .unwrap_or_default();
        let extra = row
            .get::<Option<String>, _>(6)
            .flatten()
            .unwrap_or_default();
        let unsupported = !matches!(
            ty.as_str(),
            "tinyint"
                | "smallint"
                | "mediumint"
                | "int"
                | "bigint"
                | "decimal"
                | "float"
                | "double"
                | "char"
                | "varchar"
                | "tinytext"
                | "text"
                | "mediumtext"
                | "longtext"
                | "date"
                | "datetime"
                | "timestamp"
                | "time"
                | "year"
                | "enum"
                | "set"
                | "json"
                | "bit"
                | "binary"
                | "varbinary"
                | "tinyblob"
                | "blob"
                | "mediumblob"
                | "longblob"
        ) || extra.contains("VIRTUAL GENERATED")
            || extra.contains("STORED GENERATED");
        if tables.last().is_none_or(|t| t.name != name) {
            if tables.len() >= 500 {
                return Err(AppError::policy(
                    "This preview supports at most 500 MySQL tables per database.",
                ));
            }
            tables.push(TableInfo {
                insertable: false,
                deletable: false,
                relation_kind: if row.get::<String, _>(7).as_deref() == Some("VIEW") {
                    "view".into()
                } else {
                    "table".into()
                },
                read_blocked_reason: if row.get::<String, _>(7).as_deref() != Some("BASE TABLE")
                    || row.get::<Option<String>, _>(8).flatten().as_deref() != Some("InnoDB")
                {
                    Some("Structure only: reads currently support InnoDB base tables; views and other engines remain visible here.".into())
                } else {
                    None
                },
                schema: schema.into(),
                name,
                estimated_rows: row
                    .get::<Option<u64>, _>(1)
                    .flatten()
                    .map(|v| v.min(i64::MAX as u64) as i64),
                columns: vec![],
            });
        }
        if unsupported && tables.last().unwrap().read_blocked_reason.is_none() {
            tables.last_mut().unwrap().read_blocked_reason = Some(
                "Structure only: this table has a generated column or a type not yet supported for reads.".into(),
            );
        }
        let Some(column_name) = row.get::<Option<String>, _>(2).flatten() else {
            tables.last_mut().unwrap().read_blocked_reason =
                Some("Structure only: column metadata is not accessible to this account.".into());
            continue;
        };
        tables.last_mut().unwrap().columns.push(ColumnInfo {
            insertable: false,
            has_default: false,
            type_schema: String::new(),
            name: column_name,
            data_type: ty,
            nullable: row.get::<String, _>(4).is_some_and(|v| v == "YES"),
            primary_key: row.get::<String, _>(5).is_some_and(|v| v == "PRI"),
            editable: false,
        });
    }
    Ok(tables)
}

async fn checked_catalog(
    p: &RemoteProfile,
    conn: &mut Database,
    schema: &str,
    table: Option<&str>,
) -> AppResult<Vec<TableInfo>> {
    let result = catalog(conn, schema, table).await;
    if conn.reused && result.as_ref().is_err_and(|e| e.code == "MYSQL_CONNECTION") {
        // Retry only fixed catalog SQL, before executing any user statement.
        // A failure during execution never replays the user's query.
        let previous_ms = conn.connection_ms;
        *conn = open_dedicated(p).await?;
        conn.connection_ms += previous_ms;
        return catalog(conn, schema, table).await;
    }
    result
}

pub async fn connect(p: &RemoteProfile, id: String) -> AppResult<ConnectionInfo> {
    let mut conn = open(p).await?;
    let version = conn.version.clone();
    let tables = if p.database.is_empty() {
        vec![]
    } else {
        tables(&mut conn, &p.database).await?
    };
    recycle(p, conn);
    Ok(p.info(id, version, tables))
}

async fn approved_table(
    p: &RemoteProfile,
    conn: &mut Database,
    schema: &str,
    name: &str,
) -> AppResult<TableInfo> {
    checked_catalog(p, conn, schema, Some(name)).await?.into_iter()
        .find(|t| t.name == name && t.read_blocked_reason.is_none())
        .ok_or_else(|| AppError::policy("Table is not readable in this database. Refresh its catalog to inspect supported types and permissions."))
}

pub async fn browse(p: &RemoteProfile, r: &BrowseRequest) -> AppResult<QueryResult> {
    let started = Instant::now();
    let mut conn = open(p).await?;
    let schema = selected_database(p, r.schema.as_deref())?;
    let table = approved_table(p, &mut conn, schema, &r.table).await?;
    let (sql, params) = browse_statement(&table, r, false)?;
    let validation_ms = (started.elapsed().as_millis() as u64).saturating_sub(conn.connection_ms);
    let result = read(&mut conn, sql, params, r.checked_page_size()?, true).await?;
    Ok(finish_read(p, conn, result, started, validation_ms))
}

fn browse_statement(
    table: &TableInfo,
    r: &BrowseRequest,
    count: bool,
) -> AppResult<(String, Vec<Value>)> {
    let page_size = r.checked_page_size()?;
    if r.filters.len() > 20 {
        return Err(AppError::policy("Use at most 20 filters."));
    }
    let column = |name: &str| -> AppResult<String> {
        if table.columns.iter().any(|c| c.name == name) {
            Ok(quote(name))
        } else {
            Err(AppError::policy("Unknown column."))
        }
    };
    let mut params = vec![];
    let mut conditions = vec![];
    for f in &r.filters {
        let col = column(&f.column)?;
        if f.value.len() > 4096 {
            return Err(AppError::policy("Filter value exceeds 4096 bytes."));
        }
        let expr = match f.operator {
            FilterOperator::IsNull => format!("{col} IS NULL"),
            FilterOperator::IsNotNull => format!("{col} IS NOT NULL"),
            FilterOperator::Contains => {
                params.push(Value::from(f.value.clone()));
                format!("LOCATE(?, CAST({col} AS CHAR)) > 0")
            }
            op => {
                params.push(Value::from(f.value.clone()));
                format!(
                    "{col} {} ?",
                    match op {
                        FilterOperator::Equals => "=",
                        FilterOperator::NotEquals => "<>",
                        FilterOperator::GreaterThan => ">",
                        _ => "<",
                    }
                )
            }
        };
        conditions.push(expr);
    }
    let mut sql = format!(
        "SELECT {} FROM {}.{}",
        if count { "CAST(COUNT(*) AS CHAR)" } else { "*" },
        quote(&table.schema),
        quote(&table.name)
    );
    if !conditions.is_empty() {
        sql.push_str(&format!(
            " WHERE ({})",
            conditions.join(match r.filter_match {
                FilterMatch::All => " AND ",
                FilterMatch::Any => " OR ",
            })
        ));
    }
    if count {
        return Ok((sql, params));
    }
    let mut order = vec![];
    if let Some(name) = &r.sort_column {
        let col = column(name)?;
        order.push(format!("{col} IS NULL ASC"));
        order.push(format!(
            "{col} {}",
            if r.descending { "DESC" } else { "ASC" }
        ));
    }
    for c in table
        .columns
        .iter()
        .filter(|c| c.primary_key && Some(&c.name) != r.sort_column.as_ref())
    {
        order.push(format!("{} ASC", quote(&c.name)));
    }
    if !order.is_empty() {
        sql.push_str(&format!(" ORDER BY {}", order.join(", ")));
    }
    sql.push_str(&format!(
        " LIMIT {} OFFSET {}",
        page_size + 1,
        r.page as usize * page_size
    ));
    Ok((sql, params))
}

pub async fn count(p: &RemoteProfile, r: &BrowseRequest) -> AppResult<RowCount> {
    // Counts change the session timeout and may be expensive; keep them dedicated.
    let mut conn = open_dedicated(p).await?;
    let schema = selected_database(p, r.schema.as_deref())?;
    let table = approved_table(p, &mut conn, schema, &r.table).await?;
    let (sql, params) = browse_statement(&table, r, true)?;
    conn.query_drop("SET SESSION max_execution_time = 5000")
        .await
        .map_err(db_error)?;
    let started = Instant::now();
    let rows: String = conn
        .exec_first(sql, params)
        .await
        .map_err(db_error)?
        .ok_or_else(|| AppError::policy("MySQL returned no count."))?;
    Ok(RowCount {
        rows,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

pub async fn query(p: &RemoteProfile, r: &QueryRequest) -> AppResult<QueryResult> {
    let started = Instant::now();
    let mut conn = open(p).await?;
    let tables = if let Some((schema, name)) = policy::mysql_relation(&r.sql)? {
        vec![approved_table(p, &mut conn, &schema, &name).await?]
    } else {
        vec![]
    };
    let approved = policy::approve_mysql(&r.sql, &tables)?;
    let sql = if r.explain {
        format!("EXPLAIN FORMAT=JSON {}", approved.sql())
    } else {
        approved.sql().into()
    };
    let validation_ms = (started.elapsed().as_millis() as u64).saturating_sub(conn.connection_ms);
    let result = read(&mut conn, sql, vec![], MAX_ROWS, false).await?;
    Ok(finish_read(p, conn, result, started, validation_ms))
}

fn finish_read(
    p: &RemoteProfile,
    conn: Database,
    mut result: QueryResult,
    started: Instant,
    validation_ms: u64,
) -> QueryResult {
    let mut timings = QueryTimings {
        connection_ms: conn.connection_ms,
        connection_reused: conn.reused,
        validation_ms,
        execution_ms: result.elapsed_ms,
        cleanup_ms: 0,
    };
    let cleanup = Instant::now();
    recycle(p, conn);
    timings.cleanup_ms = cleanup.elapsed().as_millis() as u64;
    result.elapsed_ms = started.elapsed().as_millis() as u64;
    result.timings = Some(timings);
    result
}

async fn read(
    conn: &mut Database,
    sql: String,
    params: Vec<Value>,
    limit: usize,
    bounded_page: bool,
) -> AppResult<QueryResult> {
    let started = Instant::now();
    conn.reusable = false;
    let mut result = conn
        .exec_iter(&sql, Params::Positional(params))
        .await
        .map_err(db_error)?;
    let columns = result
        .columns_ref()
        .iter()
        .map(|c| c.name_str().into_owned())
        .collect();
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut truncated = false;
    while let Some(row) = result.next().await.map_err(db_error)? {
        if rows.len() >= limit {
            truncated = true;
            // The application-generated page has LIMIT page_size + 1. Consume
            // its terminator after the extra row, not an unbounded SQL result.
            if bounded_page {
                let _ = result.next().await.map_err(db_error)?;
            }
            break;
        }
        let values: Vec<Option<String>> = row.unwrap().into_iter().map(text_value).collect();
        bytes += values.iter().flatten().map(String::len).sum::<usize>();
        if bytes > MAX_RESULT_BYTES {
            truncated = true;
            break;
        }
        rows.push(values);
    }
    let complete = result.is_empty();
    drop(result);
    conn.reusable = complete;
    Ok(QueryResult {
        columns,
        rows,
        elapsed_ms: started.elapsed().as_millis() as u64,
        timings: None,
        truncated,
        sql,
    })
}
fn text_value(value: Value) -> Option<String> {
    match value {
        Value::NULL => None,
        Value::Bytes(bytes) => {
            Some(String::from_utf8(bytes).unwrap_or_else(|_| "[binary value]".into()))
        }
        Value::Int(v) => Some(v.to_string()),
        Value::UInt(v) => Some(v.to_string()),
        Value::Float(v) => Some(v.to_string()),
        Value::Double(v) => Some(v.to_string()),
        Value::Date(y, m, d, h, i, s, us) => {
            Some(format!("{y:04}-{m:02}-{d:02} {h:02}:{i:02}:{s:02}.{us:06}"))
        }
        Value::Time(neg, days, h, m, s, us) => Some(format!(
            "{}{hours:02}:{m:02}:{s:02}.{us:06}",
            if neg { "-" } else { "" },
            hours = days * 24 + u32::from(h)
        )),
    }
}

pub async fn indexes(
    p: &RemoteProfile,
    table: &str,
    schema: Option<&str>,
) -> AppResult<Vec<IndexInfo>> {
    let mut conn = open(p).await?;
    let schema = selected_database(p, schema)?;
    if !checked_catalog(p, &mut conn, schema, Some(table))
        .await?
        .iter()
        .any(|t| t.name == table)
    {
        return Err(AppError::policy("Unknown table."));
    }
    let rows: Vec<Row> = conn.exec("SELECT INDEX_NAME, COLUMN_NAME, NON_UNIQUE, INDEX_TYPE, SEQ_IN_INDEX FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY INDEX_NAME,SEQ_IN_INDEX", (schema, table)).await.map_err(db_error)?;
    let mut indexes: Vec<IndexInfo> = vec![];
    for row in rows {
        let name: String = row.get(0).unwrap_or_default();
        let col: String = row.get(1).unwrap_or_else(|| "[expression]".into());
        if let Some(index) = indexes.last_mut().filter(|i| i.name == name) {
            index.definition.push_str(&format!(", {}", quote(&col)));
        } else {
            indexes.push(IndexInfo {
                primary: name == "PRIMARY",
                name,
                definition: format!(
                    "{}: {}",
                    row.get::<String, _>(3).unwrap_or_default(),
                    quote(&col)
                ),
                unique: row.get::<u64, _>(2) == Some(0),
                scans: "Not reported".into(),
                size: "Not reported".into(),
            });
        }
    }
    recycle(p, conn);
    Ok(indexes)
}

pub async fn overview(p: &RemoteProfile) -> AppResult<Overview> {
    let mut conn = open(p).await?;
    let snapshot = conn
        .query_first::<(String, String), _>(
            "SELECT VERSION(), CONCAT(CAST(UTC_TIMESTAMP(6) AS CHAR), ' UTC')",
        )
        .await
        .map_err(db_error)?
        .unwrap_or_default();
    let size: Option<String> = conn.exec_first("SELECT CAST(COALESCE(SUM(DATA_LENGTH+INDEX_LENGTH),0) AS CHAR) FROM information_schema.TABLES WHERE (?='' OR TABLE_SCHEMA=?)", (&p.database, &p.database)).await.map_err(db_error)?;
    let status: Vec<(String,String)> = conn.query("SHOW GLOBAL STATUS WHERE Variable_name IN ('Threads_connected','Threads_running','Com_commit','Com_rollback')").await.map_err(db_error)?;
    let stats: std::collections::HashMap<_, _> = status.into_iter().collect();
    let get = |name: &str| {
        stats
            .get(name)
            .cloned()
            .unwrap_or_else(|| "Not reported".into())
    };
    recycle(p, conn);
    Ok(Overview {
        database: if p.database.is_empty() {
            "Accessible databases".into()
        } else {
            p.database.clone()
        },
        server_version: snapshot.0,
        database_size: format!("{} bytes (estimate)", size.clone().unwrap_or_default()),
        database_size_bytes: size.unwrap_or_default(),
        connections: get("Threads_connected").parse().ok(),
        active_connections: get("Threads_running").parse().ok(),
        transactions_committed: get("Com_commit"),
        transactions_rolled_back: get("Com_rollback"),
        cache_hit_percent: None,
        stats_reset: None,
        observed_at: snapshot.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_profile() -> RemoteProfile {
        RemoteProfile::new(ConnectRequest {
            engine: Engine::Mysql,
            name: "Disposable fixture".into(),
            host: "127.0.0.1".into(),
            port: 13306,
            database: String::new(),
            username: "root".into(),
            password: "local-mysql-admin".into(),
            schema: String::new(),
            tls: TlsMode::LocalOnly,
            root_ca_pem: String::new(),
            manual_edits: false,
        })
        .unwrap()
    }

    async fn fixture_admin() -> Conn {
        Conn::new(
            OptsBuilder::default()
                .ip_or_hostname("127.0.0.1")
                .tcp_port(13306)
                .prefer_socket(false)
                .user(Some("root"))
                .pass(Some("local-mysql-admin")),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn privileged_mysql_connection_is_server_read_only() {
        let profile = fixture_profile();
        let mut conn = open(&profile).await.unwrap();
        let error = conn
            .query_drop("UPDATE tablepor_test.items SET label='must not write' WHERE id=1")
            .await
            .unwrap_err();
        assert!(matches!(error, mysql_async::Error::Server(e) if e.code == 1792));
        let label: Option<String> = conn
            .query_first("SELECT label FROM tablepor_test.items WHERE id=1")
            .await
            .unwrap();
        assert_eq!(label.as_deref(), Some("First"));
        // The protection must persist across implicit transactions and reuse.
        let id = conn.id();
        recycle(&profile, conn);
        let mut conn = open(&profile).await.unwrap();
        assert!(conn.reused);
        assert_eq!(conn.id(), id);
        let settings: (u8, u8) = conn
            .query_first("SELECT @@session.transaction_read_only, @@session.autocommit")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(settings, (1, 1));
        let error = conn
            .query_drop("UPDATE tablepor_test.items SET label='must not write' WHERE id=1")
            .await
            .unwrap_err();
        assert!(matches!(error, mysql_async::Error::Server(e) if e.code == 1792));
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn reused_reads_observe_new_commits_without_holding_transaction_or_metadata_locks() {
        let profile = fixture_profile();
        let mut admin = fixture_admin().await;
        let table = format!("tablepor_test.reuse_{}", uuid::Uuid::new_v4().simple());
        admin
            .query_drop(format!("CREATE TABLE {table} (id INT PRIMARY KEY)"))
            .await
            .unwrap();
        admin
            .query_drop(format!("INSERT INTO {table} VALUES (1)"))
            .await
            .unwrap();
        let mut conn = open(&profile).await.unwrap();
        let id = conn.id();
        let first = read(
            &mut conn,
            format!("SELECT id FROM {table}"),
            vec![],
            MAX_ROWS,
            false,
        )
        .await
        .unwrap();
        assert_eq!(first.rows[0][0].as_deref(), Some("1"));
        recycle(&profile, conn);
        admin
            .query_drop(format!("UPDATE {table} SET id=2"))
            .await
            .unwrap();
        let mut conn = open(&profile).await.unwrap();
        assert_eq!(conn.id(), id);
        let second = read(
            &mut conn,
            format!("SELECT id FROM {table}"),
            vec![],
            MAX_ROWS,
            false,
        )
        .await
        .unwrap();
        recycle(&profile, conn);
        // A retained explicit transaction would hold a metadata lock and block DDL.
        admin
            .query_drop("SET SESSION lock_wait_timeout=1")
            .await
            .unwrap();
        let dropped = admin.query_drop(format!("DROP TABLE {table}")).await;
        drop(profile); // Always release the test session, including on failure.
        if dropped.is_err() {
            admin
                .query_drop(format!("DROP TABLE {table}"))
                .await
                .unwrap();
        }
        assert_eq!(second.rows[0][0].as_deref(), Some("2"));
        assert!(
            dropped.is_ok(),
            "idle transport held a transaction/metadata lock"
        );
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn completed_pages_reuse_but_unbounded_and_byte_truncations_close() {
        let profile = fixture_profile();
        let mut conn = open(&profile).await.unwrap();
        let original = conn.id();
        let result = read(
            &mut conn,
            "SELECT id FROM tablepor_test.items ORDER BY id LIMIT 2".into(),
            vec![],
            1,
            true,
        )
        .await
        .unwrap();
        assert!(result.truncated && conn.reusable);
        recycle(&profile, conn);
        let mut conn = open(&profile).await.unwrap();
        assert_eq!(conn.id(), original);
        let result = read(
            &mut conn,
            "SELECT id FROM tablepor_test.items ORDER BY id".into(),
            vec![],
            1,
            false,
        )
        .await
        .unwrap();
        assert!(result.truncated && !conn.reusable);
        recycle(&profile, conn);
        let mut conn = open(&profile).await.unwrap();
        assert_ne!(conn.id(), original);
        assert!(!conn.reused);
        let result = read(
            &mut conn,
            "SELECT REPEAT('x', 2100000) UNION ALL SELECT REPEAT('x', 2100000)".into(),
            vec![],
            MAX_ROWS,
            false,
        )
        .await
        .unwrap();
        assert!(result.truncated && !conn.reusable);
        recycle(&profile, conn);
        assert!(profile.mysql_idle.lock().unwrap().is_none());
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn killed_cached_socket_recovers_before_catalog_without_replaying_user_sql() {
        let profile = fixture_profile();
        let conn = open(&profile).await.unwrap();
        let killed = conn.id();
        recycle(&profile, conn);
        let mut admin = fixture_admin().await;
        admin
            .query_drop(format!("KILL CONNECTION {killed}"))
            .await
            .unwrap();
        let mut conn = open(&profile).await.unwrap();
        let tables = checked_catalog(&profile, &mut conn, "tablepor_test", Some("items"))
            .await
            .unwrap();
        assert_eq!(tables.len(), 1);
        assert_ne!(conn.id(), killed);
        assert!(!conn.reused);
        let expired = conn.id();
        recycle(&profile, conn);
        {
            let mut idle = profile.mysql_idle.lock().unwrap();
            idle.as_mut().unwrap().idle_since = Instant::now() - Duration::from_secs(31);
        }
        admin
            .query_drop(format!("KILL CONNECTION {expired}"))
            .await
            .unwrap();
        let conn = open(&profile).await.unwrap();
        assert_ne!(conn.id(), expired);
        assert!(!conn.reused);
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn cached_statements_do_not_cache_permissions_or_table_metadata() {
        let mut admin = fixture_admin().await;
        let user = format!("reuse_{}", &uuid::Uuid::new_v4().simple().to_string()[..20]);
        admin
            .query_drop(format!(
                "CREATE USER '{user}'@'%' IDENTIFIED BY 'disposable-only'"
            ))
            .await
            .unwrap();
        admin
            .query_drop(format!(
                "GRANT SELECT ON tablepor_test.items TO '{user}'@'%'"
            ))
            .await
            .unwrap();
        let mut profile = fixture_profile();
        profile.username = user.clone();
        profile.password = "disposable-only".to_string().into();
        let request = QueryRequest {
            database: None,
            connection_id: String::new(),
            request_id: String::new(),
            schema: None,
            sql: "SELECT id FROM tablepor_test.items LIMIT 2".into(),
            explain: false,
        };
        assert_eq!(query(&profile, &request).await.unwrap().rows.len(), 2);
        assert!(
            query(&profile, &request)
                .await
                .unwrap()
                .timings
                .unwrap()
                .connection_reused
        );
        admin
            .query_drop(format!(
                "REVOKE SELECT ON tablepor_test.items FROM '{user}'@'%'"
            ))
            .await
            .unwrap();
        let denied = query(&profile, &request).await;
        drop(profile);
        admin
            .query_drop(format!("DROP USER '{user}'@'%'"))
            .await
            .unwrap();
        assert!(denied.is_err_and(|e| e.code == "POLICY_DENIED" || e.code == "MYSQL_PERMISSION"));
    }

    #[tokio::test]
    #[ignore = "requires local Compose MySQL fixture"]
    async fn cancelled_reads_close_transport_and_counts_do_not_change_cached_timeout() {
        let profile = fixture_profile();
        let conn = open(&profile).await.unwrap();
        let id = conn.id();
        recycle(&profile, conn);
        let cancelled = tokio::time::timeout(Duration::from_millis(50), async {
            let mut conn = open(&profile).await.unwrap();
            // Internal test only: user SQL policy never permits SLEEP().
            let result = read(&mut conn, "SELECT SLEEP(2)".into(), vec![], MAX_ROWS, false)
                .await
                .unwrap();
            recycle(&profile, conn);
            result
        })
        .await;
        assert!(cancelled.is_err());
        assert!(profile.mysql_idle.lock().unwrap().is_none());
        let conn = open(&profile).await.unwrap();
        assert_ne!(conn.id(), id);
        let fresh = conn.id();
        recycle(&profile, conn);
        let counted = count(
            &profile,
            &BrowseRequest {
                database: None,
                connection_id: String::new(),
                request_id: String::new(),
                schema: Some("tablepor_test".into()),
                table: "items".into(),
                filters: vec![],
                filter_match: FilterMatch::All,
                sort_column: None,
                descending: false,
                page: 0,
                page_size: 100,
            },
        )
        .await
        .unwrap();
        assert_eq!(counted.rows, "2");
        let mut conn = open(&profile).await.unwrap();
        assert_eq!(conn.id(), fresh);
        assert_eq!(
            conn.query_first::<u64, _>("SELECT @@session.max_execution_time")
                .await
                .unwrap(),
            Some(10000)
        );
    }
    #[test]
    fn server_errors_identify_the_check_and_redact_server_details() {
        for (code, expected) in [
            (1045, "MYSQL_AUTH"),
            (1044, "MYSQL_PERMISSION"),
            (1049, "MYSQL_DATABASE"),
            (1040, "MYSQL_CONNECTION_LIMIT"),
            (1130, "MYSQL_HOST"),
            (1193, "MYSQL_UNSUPPORTED"),
            (3159, "MYSQL_TLS"),
            (3024, "TIMEOUT"),
            (9999, "MYSQL_QUERY"),
        ] {
            let error = db_error(mysql_async::Error::Server(mysql_async::ServerError {
                code,
                message: "CANARY_PASSWORD_AND_QUERY_VALUE".into(),
                state: "HY000".into(),
            }));
            assert_eq!(error.code, expected);
            assert!(!error.message.contains("CANARY"));
        }
    }
}
