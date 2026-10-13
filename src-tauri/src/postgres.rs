//! PostgreSQL transport and fixed catalog queries. No UI code or generic shell access.
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use futures_util::{TryStreamExt, pin_mut};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use tokio::{sync::watch, task::JoinHandle, time::timeout};
use tokio_postgres::{Client, Config, NoTls, SimpleQueryMessage, config::SslMode};
use zeroize::Zeroizing;

use crate::{
    models::*,
    policy::{self, MAX_RESULT_BYTES, MAX_ROWS, PAGE_SIZE, quote_identifier},
};

pub struct Profile {
    pub name: String,
    host: String,
    port: u16,
    database: String,
    username: String,
    password: Zeroizing<String>,
    pub schema: String,
    tls: TlsMode,
    pub manual_edits: bool,
}

impl Profile {
    pub fn new(request: ConnectRequest) -> AppResult<Self> {
        let password = Zeroizing::new(request.password);
        for field in [
            &request.name,
            &request.host,
            &request.database,
            &request.username,
            &request.schema,
        ] {
            if field.trim().is_empty() || field.len() > 253 || field.contains('\0') {
                return Err(AppError::new(
                    "INVALID_CONNECTION",
                    "Connection fields must contain 1–253 characters and no NUL bytes.",
                ));
            }
        }
        if request.port == 0 || password.len() > 4096 {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Invalid port or password length.",
            ));
        }
        if request.tls == TlsMode::LocalOnly
            && !request
                .host
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
        {
            return Err(AppError::new(
                "TLS_REQUIRED",
                "Unencrypted connections require a literal loopback IP (127.0.0.1 or ::1). Use verified TLS for remote hosts.",
            ));
        }
        if request.host.contains('/') || request.host.contains(',') || request.host.contains(' ') {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Specify one hostname or IP address.",
            ));
        }
        Ok(Self {
            name: request.name,
            host: request.host,
            port: request.port,
            database: request.database,
            username: request.username,
            password,
            schema: request.schema,
            tls: request.tls,
            manual_edits: request.manual_edits,
        })
    }

    pub fn info(&self, id: String, version: String, tables: Vec<TableInfo>) -> ConnectionInfo {
        ConnectionInfo {
            id,
            name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            database: self.database.clone(),
            username: self.username.clone(),
            schema: self.schema.clone(),
            tls: self.tls,
            server_version: version,
            manual_edits: self.manual_edits,
            tables,
        }
    }

    pub async fn open(&self) -> AppResult<Database> {
        let mut config = Config::new();
        config.host(&self.host).port(self.port).dbname(&self.database).user(&self.username)
            .password(self.password.as_bytes()).application_name("table-por")
            .connect_timeout(Duration::from_secs(5))
            .options("-c default_transaction_read_only=on -c search_path=pg_catalog -c statement_timeout=10000 -c lock_timeout=1500 -c idle_in_transaction_session_timeout=10000 -c standard_conforming_strings=on");
        let (client, task) = match self.tls {
            TlsMode::LocalOnly => {
                config.ssl_mode(SslMode::Disable);
                let (client, connection) = timeout(Duration::from_secs(6), config.connect(NoTls))
                    .await
                    .map_err(|_| deadline())?
                    .map_err(connection_error)?;
                (
                    client,
                    tokio::spawn(async move {
                        let _ = connection.await;
                    }),
                )
            }
            TlsMode::VerifyFull => {
                config.ssl_mode(SslMode::Require);
                let (client, connection) =
                    timeout(Duration::from_secs(6), config.connect(tls_connector()?))
                        .await
                        .map_err(|_| deadline())?
                        .map_err(connection_error)?;
                (
                    client,
                    tokio::spawn(async move {
                        let _ = connection.await;
                    }),
                )
            }
        };
        let db = Database {
            client,
            task,
            tls: self.tls,
            manual_edits: self.manual_edits,
        };
        db.client
            .batch_execute("BEGIN READ ONLY")
            .await
            .map_err(query_error)?;
        Ok(db)
    }
}

pub struct Database {
    pub(crate) client: Client,
    task: JoinHandle<()>,
    tls: TlsMode,
    pub manual_edits: bool,
}

impl Drop for Database {
    fn drop(&mut self) {
        // A dedicated connection per operation: no dirty session can enter a pool.
        self.task.abort();
    }
}

fn tls_connector() -> AppResult<MakeTlsConnector> {
    let tls = TlsConnector::builder()
        .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
        .build()
        .map_err(|_| {
            AppError::new(
                "TLS_ERROR",
                "Unable to initialize the system TLS certificate verifier.",
            )
        })?;
    Ok(MakeTlsConnector::new(tls))
}

fn deadline() -> AppError {
    AppError::new(
        "TIMEOUT",
        "The operation exceeded its time limit. The dedicated connection was closed.",
    )
}

fn connection_error(error: tokio_postgres::Error) -> AppError {
    let code = error.as_db_error().map(|e| e.code().code());
    let message = match code {
        Some("28P01" | "28000") => "Authentication failed. Check the database role and password.",
        Some("3D000") => "The selected database does not exist.",
        _ => {
            "Could not connect. Check the host, port, database, network and trusted TLS certificate."
        }
    };
    AppError::new("CONNECTION_FAILED", message)
}

pub(crate) fn query_error(error: tokio_postgres::Error) -> AppError {
    // PostgreSQL error detail can contain query literals and row contents. Do not
    // serialize the raw server error or enable driver debug logging.
    match error.as_db_error().map(|e| e.code().code()) {
        Some("57014") => AppError::new(
            "QUERY_CANCELLED",
            "PostgreSQL cancelled the query or its 10-second timeout elapsed.",
        ),
        Some("42501") => AppError::new(
            "PERMISSION_DENIED",
            "The database role does not have permission for this operation.",
        ),
        Some("25006") => {
            AppError::policy("PostgreSQL rejected an operation in the read-only transaction.")
        }
        Some(code) => AppError::new(
            "DATABASE_ERROR",
            format!(
                "PostgreSQL rejected the query (SQLSTATE {code}). Check column types and supported SQL syntax."
            ),
        ),
        None => AppError::new(
            "DATABASE_ERROR",
            "The database connection ended before the result was complete.",
        ),
    }
}

impl Database {
    pub async fn preflight(&self) -> AppResult<String> {
        if !self.manual_edits {
            let role = self.client.query_one(
            "SELECT rolsuper OR rolcreaterole OR rolcreatedb OR rolreplication OR rolbypassrls AS elevated,
             EXISTS (SELECT 1 FROM pg_roles other WHERE other.oid <> me.oid AND pg_has_role(current_user, other.oid, 'MEMBER')) AS memberships,
             has_database_privilege(current_database(), 'CREATE') OR has_database_privilege(current_database(), 'TEMP') AS database_write
             FROM pg_roles me WHERE rolname = current_user", &[]).await.map_err(query_error)?;
            if role.get::<_, bool>(0) || role.get::<_, bool>(1) || role.get::<_, bool>(2) {
                return Err(AppError::new(
                    "UNSAFE_ROLE",
                    "Use a dedicated reader: no elevated roles, role memberships, database CREATE or TEMP privileges. See docs/DEVELOPMENT.md for the test reader setup.",
                ));
            }
            let unsafe_privileges = self.client.query_one(
            "SELECT
             EXISTS (SELECT 1 FROM pg_namespace WHERE has_schema_privilege(oid, 'CREATE')) OR
             EXISTS (SELECT 1 FROM pg_class c WHERE relkind IN ('r','p','v','m','f') AND c.oid <> 'pg_catalog.pg_settings'::regclass AND
               (has_table_privilege(c.oid, 'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER') OR
                has_any_column_privilege(c.oid, 'INSERT,UPDATE,REFERENCES'))) OR
             EXISTS (SELECT 1 FROM pg_class WHERE CASE WHEN relkind = 'S' THEN has_sequence_privilege(oid, 'USAGE,UPDATE') ELSE false END) OR
             EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
               WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') AND has_function_privilege(p.oid, 'EXECUTE')) AS unsafe", &[])
            .await.map_err(query_error)?;
            if unsafe_privileges.get::<_, bool>(0) {
                return Err(AppError::new(
                    "UNSAFE_ROLE",
                    "This role can write, create objects, modify sequences or execute custom routines (possibly through PUBLIC). Ask the DBA for a restricted reader; this app does not change server permissions.",
                ));
            }
        }
        let row = self
            .client
            .query_one("SHOW server_version_num", &[])
            .await
            .map_err(query_error)?;
        let version: String = row.get(0);
        if !version
            .parse::<u32>()
            .is_ok_and(|v| (170_000..180_000).contains(&v))
        {
            return Err(AppError::new(
                "UNSUPPORTED_VERSION",
                "This preview enables PostgreSQL 17. Other versions need compatibility tests before they are enabled.",
            ));
        }
        Ok(version)
    }

    pub async fn tables(&self, schema: &str) -> AppResult<Vec<TableInfo>> {
        // Exclude views, foreign/partitioned/inherited parents, RLS policies and
        // custom types; these can hide executable dependencies behind a SELECT.
        let rows = self.client.query(
            "SELECT c.oid, c.relname, c.reltuples::bigint FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relkind = 'r' AND NOT c.relrowsecurity
               AND has_table_privilege(c.oid, 'SELECT')
               AND NOT EXISTS (SELECT 1 FROM pg_inherits WHERE inhparent = c.oid)
               AND NOT EXISTS (SELECT 1 FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
                 AND a.atttypid NOT IN (16,20,21,23,25,700,701,1042,1043,1082,1083,1114,1184,1186,1266,1700,2950))
             ORDER BY c.relname LIMIT 501", &[&schema]).await.map_err(query_error)?;
        if rows.len() > 500 {
            return Err(AppError::new(
                "SCOPE_TOO_LARGE",
                "Choose a schema with at most 500 approved tables.",
            ));
        }
        let mut tables = Vec::new();
        for row in rows {
            let oid: u32 = row.get(0);
            let columns = self.client.query(
                "SELECT a.attname, t.typname, NOT a.attnotnull,
                   EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = a.attrelid AND i.indisprimary AND a.attnum = ANY(i.indkey)),
                   a.attgenerated = '' AND a.attidentity = '' AND has_column_privilege(a.attrelid, a.attnum, 'UPDATE')
                 FROM pg_attribute a JOIN pg_type t ON t.oid = a.atttypid
                 WHERE a.attrelid = $1 AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum", &[&oid])
                .await.map_err(query_error)?.into_iter().map(|c| ColumnInfo {
                    name: c.get(0), data_type: c.get(1), nullable: c.get(2), primary_key: c.get(3), editable: self.manual_edits && c.get::<_, bool>(4) && !c.get::<_, bool>(3),
                }).collect();
            let estimated: i64 = row.get(2);
            tables.push(TableInfo {
                schema: schema.into(),
                name: row.get(1),
                columns,
                estimated_rows: (estimated >= 0).then_some(estimated),
            });
        }
        Ok(tables)
    }

    async fn cancel(&self) -> bool {
        let token = self.client.cancel_token();
        let result = match self.tls {
            TlsMode::LocalOnly => timeout(Duration::from_secs(2), token.cancel_query(NoTls)).await,
            TlsMode::VerifyFull => {
                let Ok(tls) = tls_connector() else {
                    return false;
                };
                timeout(Duration::from_secs(2), token.cancel_query(tls)).await
            }
        };
        matches!(result, Ok(Ok(())))
    }

    pub async fn execute(
        &self,
        sql: &str,
        explain: bool,
        mut cancelled: watch::Receiver<bool>,
    ) -> AppResult<QueryResult> {
        let started = Instant::now();
        let sql = if explain {
            format!("EXPLAIN (FORMAT JSON) {sql}")
        } else {
            sql.to_owned()
        };
        let operation = async {
            let stream = self
                .client
                .simple_query_raw(&sql)
                .await
                .map_err(query_error)?;
            pin_mut!(stream);
            let mut result = QueryResult {
                columns: vec![],
                rows: vec![],
                elapsed_ms: 0,
                truncated: false,
                sql: sql.clone(),
            };
            let mut bytes = 0;
            while let Some(message) = stream.try_next().await.map_err(query_error)? {
                match message {
                    SimpleQueryMessage::RowDescription(columns) => {
                        result.columns = columns.iter().map(|c| c.name().to_owned()).collect()
                    }
                    SimpleQueryMessage::Row(row) => {
                        let values: Vec<Option<String>> = (0..row.len())
                            .map(|i| row.get(i).map(str::to_owned))
                            .collect();
                        bytes += values
                            .iter()
                            .map(|v| v.as_ref().map_or(4, String::len))
                            .sum::<usize>();
                        if result.rows.len() == MAX_ROWS || bytes > MAX_RESULT_BYTES {
                            result.truncated = true;
                            let _ = self.cancel().await;
                            break;
                        }
                        result.rows.push(values);
                    }
                    _ => {}
                }
            }
            result.elapsed_ms = started.elapsed().as_millis() as u64;
            Ok(result)
        };
        tokio::select! {
            result = operation => result,
            _ = async { let _ = cancelled.wait_for(|value| *value).await; } => {
                let sent = self.cancel().await;
                Err(AppError::new("CANCELLED", if sent {
                    "Cancellation requested; the dedicated connection was closed."
                } else { "The connection was closed. Server cancellation could not be confirmed; the server timeout still applies." }))
            },
            _ = tokio::time::sleep(Duration::from_secs(12)) => {
                let _ = self.cancel().await;
                Err(deadline())
            },
        }
    }

    pub async fn browse(
        &self,
        table: &TableInfo,
        request: &BrowseRequest,
    ) -> AppResult<QueryResult> {
        if request.page > 500 {
            return Err(AppError::policy(
                "Pagination is limited to 100,000 rows in this preview.",
            ));
        }
        let find_column = |name: &str| {
            table
                .columns
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| AppError::policy("Choose a column from this table."))
        };
        let projections = table
            .columns
            .iter()
            .map(|c| {
                let name = quote_identifier(&c.name);
                format!("{name}::pg_catalog.text AS {name}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "SELECT {projections} FROM {}.{}",
            quote_identifier(&table.schema),
            quote_identifier(&table.name)
        );
        if request.filters.len() > 20 {
            return Err(AppError::policy("Use at most 20 filter conditions."));
        }
        let mut parameters: Vec<String> = Vec::new();
        let mut conditions = Vec::new();
        for filter in &request.filters {
            let column = find_column(&filter.column)?;
            if filter.value.len() > 4096 {
                return Err(AppError::policy(
                    "Filter values are limited to 4,096 bytes.",
                ));
            }
            let name = quote_identifier(&column.name);
            let condition = match filter.operator {
                FilterOperator::IsNull => format!("{name} IS NULL"),
                FilterOperator::IsNotNull => format!("{name} IS NOT NULL"),
                FilterOperator::Contains => {
                    if !["text", "varchar", "bpchar"].contains(&column.data_type.as_str()) {
                        return Err(AppError::policy(
                            "Contains is available for text columns only.",
                        ));
                    }
                    parameters.push(format!(
                        "%{}%",
                        filter
                            .value
                            .replace('\\', "\\\\")
                            .replace('%', "\\%")
                            .replace('_', "\\_")
                    ));
                    format!("{name} ILIKE ${}::pg_catalog.text", parameters.len())
                }
                operator => {
                    let op = match operator {
                        FilterOperator::Equals => "=",
                        FilterOperator::NotEquals => "<>",
                        FilterOperator::GreaterThan => ">",
                        FilterOperator::LessThan => "<",
                        _ => unreachable!(),
                    };
                    parameters.push(filter.value.clone());
                    format!(
                        "{name} {op} ${}::pg_catalog.text::pg_catalog.{}",
                        parameters.len(),
                        quote_identifier(&column.data_type)
                    )
                }
            };
            conditions.push(condition);
        }
        if !conditions.is_empty() {
            let conjunction = match request.filter_match {
                FilterMatch::All => " AND ",
                FilterMatch::Any => " OR ",
            };
            sql.push_str(&format!(" WHERE ({})", conditions.join(conjunction)));
        }
        let mut order = if let Some(name) = &request.sort_column {
            vec![find_column(name)?.name.clone()]
        } else {
            table
                .columns
                .iter()
                .filter(|c| c.primary_key)
                .map(|c| c.name.clone())
                .collect()
        };
        // A stable primary-key tie breaker prevents tied sort values from moving
        // unpredictably between OFFSET pages (concurrent writes can still move rows).
        for key in table.columns.iter().filter(|c| c.primary_key) {
            if !order.contains(&key.name) {
                order.push(key.name.clone());
            }
        }
        if !order.is_empty() {
            sql.push_str(&format!(
                " ORDER BY {}",
                order
                    .iter()
                    .map(|name| format!(
                        "{}.{} {} NULLS LAST",
                        quote_identifier(&table.name),
                        quote_identifier(name),
                        if request.descending { "DESC" } else { "ASC" }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        sql.push_str(&format!(
            " LIMIT {} OFFSET {}",
            PAGE_SIZE + 1,
            request.page as usize * PAGE_SIZE
        ));
        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = parameters
            .iter()
            .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();
        let started = Instant::now();
        let stream = self
            .client
            .query_raw(&sql, params)
            .await
            .map_err(query_error)?;
        pin_mut!(stream);
        let mut result = QueryResult {
            columns: table.columns.iter().map(|c| c.name.clone()).collect(),
            rows: vec![],
            elapsed_ms: 0,
            truncated: false,
            sql,
        };
        let mut bytes = 0;
        while let Some(row) = stream.try_next().await.map_err(query_error)? {
            let values = (0..row.len())
                .map(|i| row.try_get::<_, Option<String>>(i))
                .collect::<Result<Vec<_>, _>>()
                .map_err(query_error)?;
            bytes += values
                .iter()
                .map(|v| v.as_ref().map_or(4, String::len))
                .sum::<usize>();
            if result.rows.len() == PAGE_SIZE || bytes > MAX_RESULT_BYTES {
                result.truncated = true;
                let _ = self.cancel().await;
                break;
            }
            result.rows.push(values);
        }
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }

    pub async fn indexes(&self, schema: &str, table: &str) -> AppResult<Vec<IndexInfo>> {
        self.client.query(
            "SELECT ci.relname, pg_get_indexdef(i.indexrelid), COALESCE(s.idx_scan,0)::text,
             pg_size_pretty(pg_relation_size(ci.oid)), i.indisunique, i.indisprimary
             FROM pg_index i JOIN pg_class ct ON ct.oid = i.indrelid
             JOIN pg_namespace n ON n.oid = ct.relnamespace JOIN pg_class ci ON ci.oid = i.indexrelid
             LEFT JOIN pg_stat_user_indexes s ON s.indexrelid = i.indexrelid
             WHERE n.nspname = $1 AND ct.relname = $2 AND has_table_privilege(ct.oid, 'SELECT')
             ORDER BY ci.relname LIMIT 100", &[&schema, &table]).await.map_err(query_error)
            .map(|rows| rows.into_iter().map(|r| IndexInfo { name: r.get(0), definition: r.get(1), scans: r.get(2), size: r.get(3), unique: r.get(4), primary: r.get(5) }).collect())
    }

    pub async fn overview(&self) -> AppResult<Overview> {
        let row = self.client.query_one(
            "SELECT current_database(), current_setting('server_version'), pg_size_pretty(pg_database_size(current_database())),
             numbackends::bigint, (SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND state = 'active'),
             xact_commit::text, xact_rollback::text,
             CASE WHEN blks_hit + blks_read > 0 THEN 100.0 * blks_hit::float8 / (blks_hit::float8 + blks_read::float8) ELSE NULL END,
             stats_reset::text, clock_timestamp()::text FROM pg_stat_database WHERE datname = current_database()", &[])
            .await.map_err(query_error)?;
        Ok(Overview {
            database: row.get(0),
            server_version: row.get(1),
            database_size: row.get(2),
            connections: row.get(3),
            active_connections: row.get(4),
            transactions_committed: row.get(5),
            transactions_rolled_back: row.get(6),
            cache_hit_percent: row.get(7),
            stats_reset: row.get(8),
            observed_at: row.get(9),
        })
    }

    pub async fn run_validated(
        &self,
        sql: &str,
        schema: &str,
        explain: bool,
        cancelled: watch::Receiver<bool>,
    ) -> AppResult<QueryResult> {
        self.preflight().await?;
        let tables = self.tables(schema).await?;
        let approved = policy::approve(sql, &tables)?;
        self.execute(approved.sql(), explain, cancelled).await
    }

    pub async fn stop(&self) {
        let _ = self.cancel().await;
    }
}
