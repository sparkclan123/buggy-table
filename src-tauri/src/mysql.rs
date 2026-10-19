//! MySQL 8 reads use a SELECT-only role, prepared values and dedicated connections.
use crate::{
    models::*,
    policy::{self, MAX_RESULT_BYTES, MAX_ROWS},
    remote_profile::RemoteProfile,
};
use mysql_async::{Conn, OptsBuilder, Params, Row, SslOpts, Value, prelude::Queryable};
use std::time::Instant;

pub fn quote(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}
fn db_error(error: mysql_async::Error) -> AppError {
    use mysql_async::{DriverError, Error, IoError};
    let (code, message) = match error {
        Error::Io(IoError::Tls(_)) => (
            "MYSQL_TLS",
            "MySQL TLS verification failed. Check the certificate hostname and expiry; for a private CA, choose its PEM file. Plaintext fallback is disabled.",
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
pub struct Database(Option<Conn>);
impl std::ops::Deref for Database {
    type Target = Conn;
    fn deref(&self) -> &Conn {
        self.0.as_ref().expect("live connection")
    }
}
impl std::ops::DerefMut for Database {
    fn deref_mut(&mut self) -> &mut Conn {
        self.0.as_mut().expect("live connection")
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if let Some(conn) = self.0.take()
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
    let tls = SslOpts::default().with_root_certs(if p.root_ca_pem.is_empty() {
        vec![]
    } else {
        vec![p.root_ca_pem.as_bytes().to_vec().into()]
    });
    let opts = OptsBuilder::default()
        .ip_or_hostname(&p.host)
        .tcp_port(p.port)
        .user(Some(p.username.clone()))
        .pass(Some(p.password.to_string()))
        .db_name(Some(p.database.clone()))
        .prefer_socket(false)
        .enable_cleartext_plugin(false)
        .max_allowed_packet(Some(MAX_RESULT_BYTES))
        .ssl_opts((p.tls == TlsMode::VerifyFull).then_some(tls));
    let mut conn = Database(Some(Conn::new(opts).await.map_err(db_error)?));
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
    // Reject roles/proxy, global admin privileges, FILE, EXECUTE and GRANT OPTION.
    // Rechecked on every operation, so privilege drift is not silently accepted.
    let grants: Vec<String> = conn
        .query("SHOW GRANTS FOR CURRENT_USER()")
        .await
        .map_err(db_error)?;
    if grants.is_empty() || grants.iter().any(|g| !safe_grant(g)) {
        return Err(AppError::policy(
            "Use a MySQL account with only direct SELECT/SHOW VIEW grants (no roles, writes, EXECUTE, FILE or GRANT OPTION).",
        ));
    }
    conn.query_drop("SET SESSION sql_mode = 'STRICT_TRANS_TABLES,NO_BACKSLASH_ESCAPES', max_execution_time = 10000, lock_wait_timeout = 2").await.map_err(db_error)?;
    conn.query_drop("START TRANSACTION READ ONLY")
        .await
        .map_err(db_error)?;
    Ok(conn)
}

fn safe_grant(grant: &str) -> bool {
    let Some(rest) = grant.strip_prefix("GRANT ") else {
        return false;
    };
    let Some((privileges, target)) = rest.split_once(" ON ") else {
        return false;
    };
    !target.contains(" WITH GRANT OPTION")
        && privileges
            .split(", ")
            .all(|p| matches!(p, "USAGE" | "SELECT" | "SHOW VIEW"))
}

pub async fn tables(conn: &mut Conn, schema: &str) -> AppResult<Vec<TableInfo>> {
    let records: Vec<Row> = conn.exec("SELECT t.TABLE_NAME, t.TABLE_ROWS, c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE, c.COLUMN_KEY, c.EXTRA FROM information_schema.TABLES t JOIN information_schema.COLUMNS c ON c.TABLE_SCHEMA=t.TABLE_SCHEMA AND c.TABLE_NAME=t.TABLE_NAME WHERE t.TABLE_SCHEMA=? AND t.TABLE_TYPE='BASE TABLE' AND t.ENGINE='InnoDB' ORDER BY t.TABLE_NAME,c.ORDINAL_POSITION LIMIT 16385", (schema,)).await.map_err(db_error)?;
    if records.len() > 16384 {
        return Err(AppError::policy(
            "MySQL catalog exceeds the 16,384-column metadata limit.",
        ));
    }
    let mut tables: Vec<TableInfo> = Vec::new();
    let mut excluded = std::collections::HashSet::new();
    for row in records {
        let name: String = row.get(0).unwrap_or_default();
        let ty: String = row.get(3).unwrap_or_default();
        let extra: String = row.get(6).unwrap_or_default();
        if !matches!(
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
        ) || extra.contains("GENERATED")
        {
            excluded.insert(name.clone());
        }
        if tables.last().is_none_or(|t| t.name != name) {
            if tables.len() >= 500 {
                return Err(AppError::policy(
                    "This preview supports at most 500 MySQL tables per database.",
                ));
            }
            tables.push(TableInfo {
                deletable: false,
                relation_kind: "table".into(),
                read_blocked_reason: None,
                schema: schema.into(),
                name,
                estimated_rows: row
                    .get::<Option<u64>, _>(1)
                    .flatten()
                    .map(|v| v.min(i64::MAX as u64) as i64),
                columns: vec![],
            });
        }
        tables.last_mut().unwrap().columns.push(ColumnInfo {
            type_schema: String::new(),
            name: row.get(2).unwrap_or_default(),
            data_type: ty,
            nullable: row.get::<String, _>(4).is_some_and(|v| v == "YES"),
            primary_key: row.get::<String, _>(5).is_some_and(|v| v == "PRI"),
            editable: false,
        });
    }
    tables.retain(|t| !excluded.contains(&t.name));
    Ok(tables)
}

pub async fn connect(p: &RemoteProfile, id: String) -> AppResult<ConnectionInfo> {
    let mut conn = open(p).await?;
    let version = conn
        .query_first::<String, _>("SELECT VERSION()")
        .await
        .map_err(db_error)?
        .unwrap_or_default();
    let tables = tables(&mut conn, &p.database).await?;
    Ok(p.info(id, version, tables))
}

async fn approved_tables(conn: &mut Conn, schema: &str) -> AppResult<Vec<TableInfo>> {
    tables(conn, schema).await
}

pub async fn browse(p: &RemoteProfile, r: &BrowseRequest) -> AppResult<QueryResult> {
    let mut conn = open(p).await?;
    let tables = approved_tables(&mut conn, &p.database).await?;
    let table = tables
        .iter()
        .find(|t| t.name == r.table)
        .ok_or_else(|| AppError::policy("Table is outside the supported MySQL scope."))?;
    let (sql, params) = browse_statement(p, table, r, false)?;
    read(&mut conn, sql, params, r.checked_page_size()?).await
}

fn browse_statement(
    p: &RemoteProfile,
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
        quote(&p.database),
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
    let mut conn = open(p).await?;
    let tables = approved_tables(&mut conn, &p.database).await?;
    let table = tables
        .iter()
        .find(|t| t.name == r.table)
        .ok_or_else(|| AppError::policy("Table is outside the supported MySQL scope."))?;
    let (sql, params) = browse_statement(p, table, r, true)?;
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
    let mut conn = open(p).await?;
    let tables = approved_tables(&mut conn, &p.database).await?;
    let approved = policy::approve_mysql(&r.sql, &tables)?;
    let sql = if r.explain {
        format!("EXPLAIN FORMAT=JSON {}", approved.sql())
    } else {
        approved.sql().into()
    };
    read(&mut conn, sql, vec![], MAX_ROWS).await
}

async fn read(
    conn: &mut Conn,
    sql: String,
    params: Vec<Value>,
    limit: usize,
) -> AppResult<QueryResult> {
    let started = Instant::now();
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
    // Dropping the dedicated connection avoids draining arbitrarily large results.
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

pub async fn indexes(p: &RemoteProfile, table: &str) -> AppResult<Vec<IndexInfo>> {
    let mut conn = open(p).await?;
    if !tables(&mut conn, &p.database)
        .await?
        .iter()
        .any(|t| t.name == table)
    {
        return Err(AppError::policy("Unknown table."));
    }
    let rows: Vec<Row> = conn.exec("SELECT INDEX_NAME, COLUMN_NAME, NON_UNIQUE, INDEX_TYPE, SEQ_IN_INDEX FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY INDEX_NAME,SEQ_IN_INDEX", (&p.database, table)).await.map_err(db_error)?;
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
    let size: Option<String> = conn.exec_first("SELECT CAST(COALESCE(SUM(DATA_LENGTH+INDEX_LENGTH),0) AS CHAR) FROM information_schema.TABLES WHERE TABLE_SCHEMA=?", (&p.database,)).await.map_err(db_error)?;
    let status: Vec<(String,String)> = conn.query("SHOW GLOBAL STATUS WHERE Variable_name IN ('Threads_connected','Threads_running','Com_commit','Com_rollback')").await.map_err(db_error)?;
    let stats: std::collections::HashMap<_, _> = status.into_iter().collect();
    let get = |name: &str| {
        stats
            .get(name)
            .cloned()
            .unwrap_or_else(|| "Not reported".into())
    };
    Ok(Overview {
        database: p.database.clone(),
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
    #[test]
    fn grants_fail_closed() {
        assert!(safe_grant("GRANT SELECT ON `app`.* TO `reader`@`%`"));
        for g in [
            "GRANT ALL PRIVILEGES ON *.* TO `root`@`%`",
            "GRANT SELECT, EXECUTE ON *.* TO x",
            "GRANT SELECT ON *.* TO x WITH GRANT OPTION",
            "GRANT `reader_role`@`%` TO x",
            "GRANT PROXY ON x TO y",
        ] {
            assert!(!safe_grant(g));
        }
    }
}
