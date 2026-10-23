//! PostgreSQL transport and fixed catalog queries. No UI code or generic shell access.
use std::{
    collections::{HashMap, VecDeque},
    error::Error as _,
    io::ErrorKind,
    net::IpAddr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::{TryStreamExt, pin_mut};
use native_tls::{Certificate, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use tokio::{sync::watch, task::JoinHandle, time::timeout};
use tokio_postgres::{Client, Config, NoTls, SimpleQueryMessage, config::SslMode, types::Type};
use zeroize::Zeroizing;

use crate::{
    models::*,
    policy::{self, MAX_RESULT_BYTES, MAX_ROWS, quote_identifier},
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
    root_ca: Vec<Certificate>,
    pub manual_edits: bool,
    idle: Mutex<VecDeque<Database>>,
    discovered_database: OnceLock<String>,
    pub(crate) endpoint: Option<std::net::SocketAddr>,
}

impl Profile {
    pub fn new(request: ConnectRequest) -> AppResult<Self> {
        let password = Zeroizing::new(request.password);
        if request.tls != TlsMode::VerifyFull && !request.root_ca_pem.is_empty() {
            return Err(AppError::new(
                "INVALID_CA",
                "A CA certificate requires verified TLS mode.",
            ));
        }
        let root_ca = parse_root_ca(&request.root_ca_pem)?;
        for field in [
            &request.name,
            &request.host,
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
        if request.database.len() > 253 || request.database.contains('\0') {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Invalid database name.",
            ));
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
            root_ca,
            manual_edits: request.manual_edits,
            idle: Mutex::new(VecDeque::new()),
            discovered_database: OnceLock::new(),
            endpoint: None,
        })
    }

    pub fn info(&self, id: String, version: String, tables: Vec<TableInfo>) -> ConnectionInfo {
        ConnectionInfo {
            engine: Engine::Postgres,
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
            vpn: crate::vpn::VpnSelection::default(),
            vpn_owner: None,
        }
    }

    /// At most four idle transports per server workspace, keyed by database.
    /// Approved reads use PostgreSQL's implicit
    /// read-only transactions, so successful reads leave no transaction open.
    pub async fn open(&self) -> AppResult<Database> {
        self.open_in(None).await
    }

    pub async fn open_in(&self, database: Option<&str>) -> AppResult<Database> {
        let target = self.target_database(database)?;
        let Some(target) = target else {
            return self.discover().await;
        };
        let idle = self.idle.lock().ok().and_then(|mut idle| {
            let index = idle.iter().position(|db| db.database == target)?;
            idle.remove(index)
        });
        if let Some(mut db) = idle
            && !db.client.is_closed()
        {
            db.reused = true;
            return Ok(db);
        }
        self.connect_transport(&target).await
    }

    pub fn recycle(&self, db: Database) {
        // Only fully consumed, successful, session-neutral reads are returned.
        // Saves/counts use dedicated transactions; errors/cancellation close the socket.
        if db.reusable.load(Ordering::Relaxed)
            && !db.client.is_closed()
            && let Ok(mut idle) = self.idle.lock()
        {
            idle.retain(|old| old.database != db.database && !old.client.is_closed());
            idle.push_back(db);
            while idle.len() > 4 {
                idle.pop_front();
            }
        }
    }

    pub async fn open_dedicated_in(&self, database: Option<&str>) -> AppResult<Database> {
        let db = match self.target_database(database)? {
            Some(target) => self.connect_transport(&target).await?,
            None => self.discover().await?,
        };
        db.prevent_reuse();
        Ok(db)
    }

    fn target_database(&self, database: Option<&str>) -> AppResult<Option<String>> {
        if let Some(name) = database {
            if name.is_empty() || name.len() > 253 || name.contains('\0') {
                return Err(AppError::new(
                    "INVALID_CONNECTION",
                    "Invalid database name.",
                ));
            }
            return Ok(Some(name.into()));
        }
        Ok(if self.database.is_empty() {
            self.discovered_database.get().cloned()
        } else {
            Some(self.database.clone())
        })
    }

    async fn discover(&self) -> AppResult<Database> {
        // PostgreSQL requires a database in its startup packet. Try only conventional
        // maintenance databases; never retry an authentication/TLS/network failure.
        let mut candidates = vec!["postgres", self.username.as_str(), "template1"];
        candidates.dedup();
        for name in candidates {
            match self.connect_transport(name).await {
                Ok(db) => {
                    let _ = self.discovered_database.set(name.into());
                    return Ok(db);
                }
                Err(e) if matches!(e.code, "DATABASE_UNAVAILABLE") => continue,
                Err(e) => return Err(e),
            }
        }
        Err(AppError::new(
            "DATABASE_REQUIRED",
            "No accessible maintenance database was found. Enter a database name your account can connect to; you can browse other databases after connecting.",
        ))
    }

    async fn connect_transport(&self, database: &str) -> AppResult<Database> {
        let mut config = Config::new();
        config.host(&self.host).port(self.endpoint.map_or(self.port, |e| e.port())).dbname(database).user(&self.username)
            .password(self.password.as_bytes()).application_name("table-por")
            .connect_timeout(Duration::from_secs(5))
            .options("-c default_transaction_read_only=on -c search_path=pg_catalog -c statement_timeout=10000 -c lock_timeout=1500 -c idle_in_transaction_session_timeout=10000 -c standard_conforming_strings=on");
        if let Some(endpoint) = self.endpoint {
            // host remains the original DB hostname for TLS identity verification.
            config.hostaddr(endpoint.ip());
        }
        let (client, task) = match self.tls {
            TlsMode::LocalOnly => {
                config.ssl_mode(SslMode::Disable);
                let (client, connection) = timeout(Duration::from_secs(6), config.connect(NoTls))
                    .await
                    .map_err(|_| deadline())?
                    .map_err(|error| connection_error(error, self.tls))?;
                (
                    client,
                    tokio::spawn(async move {
                        let _ = connection.await;
                    }),
                )
            }
            TlsMode::VerifyFull | TlsMode::Require => {
                config.ssl_mode(SslMode::Require);
                let (client, connection) = timeout(
                    Duration::from_secs(6),
                    config.connect(MakeTlsConnector::new(tls_connector(
                        self.tls,
                        &self.root_ca,
                    )?)),
                )
                .await
                .map_err(|_| deadline())?
                .map_err(|error| connection_error(error, self.tls))?;
                (
                    client,
                    tokio::spawn(async move {
                        let _ = connection.await;
                    }),
                )
            }
        };
        let db = Database {
            database: database.into(),
            client,
            task,
            tls: self.tls,
            root_ca: self.root_ca.clone(),
            manual_edits: self.manual_edits,
            reused: false,
            reusable: AtomicBool::new(true),
            version: OnceLock::new(),
        };
        Ok(db)
    }
}

pub struct Database {
    database: String,
    pub(crate) client: Client,
    task: JoinHandle<()>,
    tls: TlsMode,
    root_ca: Vec<Certificate>,
    pub manual_edits: bool,
    pub reused: bool,
    reusable: AtomicBool,
    version: OnceLock<String>,
}

impl Drop for Database {
    fn drop(&mut self) {
        // Errors, interrupted work and discarded sessions close the transport immediately.
        self.task.abort();
    }
}

pub(crate) fn parse_root_ca(pem: &str) -> AppResult<Vec<Certificate>> {
    let invalid = || {
        AppError::new(
            "INVALID_CA",
            "Choose a PEM CA certificate file, up to 64 KiB and 16 certificates. Private keys are not accepted.",
        )
    };
    if pem.len() > 64 * 1024 {
        return Err(invalid());
    }
    let mut certificates = Vec::new();
    let mut remaining = pem.trim();
    while !remaining.is_empty() {
        if certificates.len() == 16 || !remaining.starts_with("-----BEGIN CERTIFICATE-----") {
            return Err(invalid());
        }
        let end = remaining
            .find("-----END CERTIFICATE-----")
            .ok_or_else(invalid)?
            + "-----END CERTIFICATE-----".len();
        certificates
            .push(Certificate::from_pem(&remaining.as_bytes()[..end]).map_err(|_| invalid())?);
        remaining = remaining[end..].trim();
    }
    Ok(certificates)
}

fn tls_connector(mode: TlsMode, root_ca: &[Certificate]) -> AppResult<TlsConnector> {
    let mut builder = TlsConnector::builder();
    builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
    // Explicit compatibility mode: still require encryption, never plaintext fallback.
    if mode == TlsMode::Require {
        builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
    }
    for certificate in root_ca {
        builder.add_root_certificate(certificate.clone());
    }
    let tls = builder.build().map_err(|_| {
        AppError::new(
            "TLS_ERROR",
            "Unable to initialize the system TLS certificate verifier.",
        )
    })?;
    Ok(tls)
}

fn deadline() -> AppError {
    AppError::new(
        "TIMEOUT",
        "The operation exceeded its time limit. The connection was closed.",
    )
}

fn connection_error(error: tokio_postgres::Error, mode: TlsMode) -> AppError {
    let code = error.as_db_error().map(|e| e.code().code());
    if matches!(code, Some("3D000" | "42501")) {
        return AppError::new(
            "DATABASE_UNAVAILABLE",
            "The database does not exist or this account cannot connect to it. Choose an accessible database.",
        );
    }
    // Use driver categories and typed causes, never expose raw errors or server details.
    if error.to_string() == "error performing TLS handshake" {
        let unsupported = error
            .source()
            .is_some_and(|cause| cause.to_string() == "server does not support TLS");
        return AppError::new(
            "TLS_CONNECTION_FAILED",
            if unsupported {
                "The server does not support TLS. No password was sent. Enable TLS on the server or use a trusted local tunnel."
            } else if mode == TlsMode::VerifyFull {
                "TLS handshake or certificate verification failed. For a private CA, choose the provider's CA PEM file and use a Host that matches the certificate. SSL Preferred in other clients does not enforce this verification."
            } else {
                "The TLS handshake failed. Check the server's TLS configuration and whether it requires a client certificate. Plaintext fallback is disabled."
            },
        );
    }
    let mut cause = error.source();
    while let Some(current) = cause {
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            let message = match io.kind() {
                ErrorKind::ConnectionRefused => {
                    "The server refused the connection. Check the port and that PostgreSQL is listening."
                }
                ErrorKind::TimedOut => {
                    "The connection timed out. Check your network, VPN and firewall allowlist."
                }
                _ => {
                    "Could not reach the database server. Check the hostname, port, network and firewall allowlist."
                }
            };
            return AppError::new("NETWORK_ERROR", message);
        }
        cause = current.source();
    }
    let message = match code {
        Some("28P01" | "28000") => "Authentication failed. Check the database role and password.",
        Some("3D000") => {
            "The selected database does not exist. Enter the database name; public is usually a schema name."
        }
        Some("53300") => "The server has reached its connection limit. Try again later.",
        Some("57P03") => "The database server is starting up or unavailable. Try again later.",
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
        Some("57P01" | "57P02" | "57P03" | "08000" | "08003" | "08006") => AppError::new(
            "CONNECTION_LOST",
            "The database connection closed. Reconnect or run a new read; an interrupted query is never replayed.",
        ),
        None if error.is_closed() => AppError::new(
            "CONNECTION_LOST",
            "The database connection closed. Reconnect or run a new read; an interrupted query is never replayed.",
        ),
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

// No upper gate: future PostgreSQL releases may connect, with a UI notice until tested.
fn supported_scalar(oid: u32) -> bool {
    // Built-in scalar I/O only. Enums and their arrays are checked separately;
    // domains/composites/extensions never inherit approval just by their names.
    matches!(
        oid,
        16 | 17
            | 20
            | 21
            | 23
            | 25
            | 114
            | 650
            | 700
            | 701
            | 790
            | 829
            | 869
            | 1042
            | 1043
            | 1082
            | 1083
            | 1114
            | 1184
            | 1186
            | 1266
            | 1700
            | 2950
            | 3802
    )
}

pub(crate) fn column_type_sql(column: &ColumnInfo) -> String {
    format!(
        "{}.{}",
        quote_identifier(&column.type_schema),
        quote_identifier(&column.data_type)
    )
}

pub(crate) fn text_comparison(column: &ColumnInfo) -> bool {
    column.type_schema == "pg_catalog" && matches!(column.data_type.as_str(), "json" | "_json")
}

fn validate_server_version(version: &str) -> AppResult<()> {
    if !version.parse::<u32>().is_ok_and(|v| v >= 150_000) {
        return Err(AppError::new(
            "UNSUPPORTED_VERSION",
            "PostgreSQL 15 or newer is required. Versions 15–18 are tested; newer versions can connect with compatibility unverified.",
        ));
    }
    Ok(())
}

impl Database {
    pub(crate) fn prevent_reuse(&self) {
        self.reusable.store(false, Ordering::Relaxed);
    }

    pub async fn preflight(&self) -> AppResult<String> {
        // The server version is immutable for this physical connection. Privileges are not.
        if self.manual_edits
            && let Some(version) = self.version.get()
        {
            return Ok(version.clone());
        }
        let version: String = if self.manual_edits {
            self.client
                .query_typed_one("SHOW server_version_num", &[])
                .await
                .map_err(query_error)?
                .get(0)
        } else {
            let role = self.client.query_typed_one(
            "SELECT current_setting('server_version_num'),
             rolsuper OR rolcreaterole OR rolcreatedb OR rolreplication OR rolbypassrls,
             EXISTS (SELECT 1 FROM pg_roles other WHERE other.oid <> me.oid AND pg_has_role(current_user, other.oid, 'MEMBER')),
             has_database_privilege(current_database(), 'CREATE') OR has_database_privilege(current_database(), 'TEMP'),
             EXISTS (SELECT 1 FROM pg_namespace WHERE has_schema_privilege(oid, 'CREATE')) OR
             EXISTS (SELECT 1 FROM pg_class c WHERE relkind IN ('r','p','v','m','f') AND c.oid <> 'pg_catalog.pg_settings'::regclass AND
               (has_table_privilege(c.oid, 'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER') OR
                has_any_column_privilege(c.oid, 'INSERT,UPDATE,REFERENCES'))) OR
             EXISTS (SELECT 1 FROM pg_class WHERE CASE WHEN relkind = 'S' THEN has_sequence_privilege(oid, 'USAGE,UPDATE') ELSE false END) OR
             EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
               WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') AND has_function_privilege(p.oid, 'EXECUTE'))
             FROM pg_roles me WHERE rolname = current_user", &[]).await.map_err(query_error)?;
            if role.get::<_, bool>(1) || role.get::<_, bool>(2) || role.get::<_, bool>(3) {
                return Err(AppError::new(
                    "UNSAFE_ROLE",
                    "Use a dedicated reader: no elevated roles, role memberships, database CREATE or TEMP privileges. See docs/DEVELOPMENT.md for the test reader setup.",
                ));
            }
            if role.get::<_, bool>(4) {
                return Err(AppError::new(
                    "UNSAFE_ROLE",
                    "This role can write, create objects, modify sequences or execute custom routines (possibly through PUBLIC). Ask the DBA for a restricted reader; this app does not change server permissions.",
                ));
            }
            role.get(0)
        };
        validate_server_version(&version)?;
        let _ = self.version.set(version.clone());
        Ok(version)
    }

    pub async fn databases(&self) -> AppResult<Vec<String>> {
        let rows = self.client.query_typed("SELECT datname FROM pg_database WHERE datallowconn AND NOT datistemplate AND datconnlimit <> -2 AND has_database_privilege(oid, 'CONNECT') ORDER BY datname LIMIT 1001", &[]).await.map_err(query_error)?;
        if rows.len() > 1000 {
            return Err(AppError::new(
                "CATALOG_LIMIT",
                "This server exposes more than 1000 databases. The list was not partially loaded.",
            ));
        }
        Ok(rows.iter().map(|row| row.get(0)).collect())
    }

    pub async fn schemas(&self) -> AppResult<Vec<String>> {
        let rows = self.client.query_typed("SELECT nspname FROM pg_namespace WHERE nspname !~ '^pg_' AND nspname <> 'information_schema' AND has_schema_privilege(oid, 'USAGE') ORDER BY nspname LIMIT 1001", &[]).await.map_err(query_error)?;
        if rows.len() > 1000 {
            return Err(AppError::new(
                "SCOPE_TOO_LARGE",
                "This database exposes more than 1000 schemas.",
            ));
        }
        Ok(rows.into_iter().map(|r| r.get(0)).collect())
    }

    pub async fn tables(&self, schema: &str) -> AppResult<Vec<TableInfo>> {
        self.catalog(schema, None).await
    }

    pub async fn approved_table(&self, schema: &str, name: &str) -> AppResult<TableInfo> {
        self.catalog(schema, Some(name)).await?.into_iter()
            .find(|table| table.read_blocked_reason.is_none())
            .ok_or_else(|| AppError::policy("This table is outside the approved scope. Refresh the schema to inspect its permissions and type restrictions."))
    }

    async fn catalog(&self, schema: &str, table_name: Option<&str>) -> AppResult<Vec<TableInfo>> {
        if schema.is_empty() || schema.len() > 253 || schema.contains('\0') {
            return Err(AppError::policy("Invalid schema name."));
        }
        let schema_params: &[(&(dyn tokio_postgres::types::ToSql + Sync), Type)] =
            &[(&schema, Type::TEXT)];
        let scope_params: &[(&(dyn tokio_postgres::types::ToSql + Sync), Type)] =
            &[(&schema, Type::TEXT), (&table_name, Type::TEXT)];
        // Independent catalog reads are pipelined on one connection. Their results
        // never authorize execution until every permission/type check has passed.
        let accessible = self.client.query_typed_one("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1 AND nspname !~ '^pg_' AND nspname <> 'information_schema' AND has_schema_privilege(oid, 'USAGE'))", schema_params);
        let relations = self.client.query_typed(
            "SELECT c.oid, c.relname, c.reltuples::bigint, c.relkind::text, c.relrowsecurity,
               has_table_privilege(c.oid, 'SELECT'),
               EXISTS (SELECT 1 FROM pg_inherits WHERE inhparent = c.oid),
               has_table_privilege(c.oid, 'DELETE'),
               has_table_privilege(c.oid, 'INSERT') OR has_any_column_privilege(c.oid, 'INSERT')
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relkind IN ('r','p','v','m','f')
               AND ($2::text IS NULL OR c.relname = $2)
             ORDER BY c.relname LIMIT 5001",
            scope_params,
        );
        let columns = self.client.query_typed(
            "SELECT a.attrelid, a.attname, t.typname, NOT a.attnotnull,
               EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = a.attrelid AND i.indisprimary AND a.attnum = ANY(i.indkey)),
               a.attgenerated = '' AND a.attidentity = '' AND has_column_privilege(a.attrelid, a.attnum, 'UPDATE'),
               n.nspname, t.oid, t.typtype::text, t.typelem, COALESCE(e.typtype::text, ''),
               t.typinput = 'pg_catalog.array_in'::regproc AND t.typoutput = 'pg_catalog.array_out'::regproc,
               a.attgenerated::text,
               a.attgenerated = '' AND a.attidentity = '' AND has_column_privilege(a.attrelid, a.attnum, 'INSERT'),
               a.atthasdef OR a.attidentity <> '' OR a.attgenerated <> ''
             FROM pg_attribute a JOIN pg_type t ON t.oid = a.atttypid
             JOIN pg_namespace n ON n.oid = t.typnamespace
             LEFT JOIN pg_type e ON e.oid = t.typelem
             JOIN pg_class c ON c.oid = a.attrelid
             JOIN pg_namespace scope ON scope.oid = c.relnamespace
             WHERE scope.nspname = $1 AND c.relkind IN ('r','p','v','m','f')
               AND ($2::text IS NULL OR c.relname = $2)
               AND a.attnum > 0 AND NOT a.attisdropped
             ORDER BY a.attrelid, a.attnum LIMIT 50001", scope_params);
        let (accessible, rows, columns) =
            tokio::try_join!(accessible, relations, columns).map_err(query_error)?;
        if !accessible.get::<_, bool>(0) {
            return Err(AppError::new(
                "SCHEMA_UNAVAILABLE",
                "Schema does not exist or this account has no USAGE permission.",
            ));
        }
        if rows.len() > 5000 {
            return Err(AppError::new(
                "SCOPE_TOO_LARGE",
                "This schema exceeds 5,000 tables/views. The catalog was not partially loaded.",
            ));
        }
        if columns.len() > 50000 {
            return Err(AppError::new(
                "SCOPE_TOO_LARGE",
                "This schema exceeds 50,000 columns. The catalog was not partially loaded.",
            ));
        }
        let mut by_table: HashMap<u32, Vec<ColumnInfo>> = HashMap::new();
        let mut blocked: HashMap<u32, String> = HashMap::new();
        for column in columns {
            let oid: u32 = column.get(0);
            let type_oid: u32 = column.get(7);
            let kind: String = column.get(8);
            let element_oid: u32 = column.get(9);
            let element_kind: String = column.get(10);
            let supported = supported_scalar(type_oid)
                || kind == "e"
                || (column.get::<_, bool>(11)
                    && (supported_scalar(element_oid) || element_kind == "e"));
            let generated: String = column.get(12);
            if !supported {
                blocked.entry(oid).or_insert_with(|| format!("Column {} uses unsupported type {}.{}. Structure is available; reading this type is not enabled.", column.get::<_, String>(1), column.get::<_, String>(6), column.get::<_, String>(2)));
            } else if !matches!(generated.as_str(), "" | "s") {
                blocked.entry(oid).or_insert_with(|| "Virtual generated columns can execute expressions when read. Structure is available; reading is not enabled.".into());
            }
            by_table.entry(column.get(0)).or_default().push(ColumnInfo {
                name: column.get(1),
                data_type: column.get(2),
                type_schema: column.get(6),
                nullable: column.get(3),
                primary_key: column.get(4),
                insertable: self.manual_edits && column.get::<_, bool>(13),
                has_default: column.get(14),
                editable: self.manual_edits
                    && column.get::<_, bool>(5)
                    && !column.get::<_, bool>(4),
            });
        }
        let mut tables = Vec::with_capacity(rows.len());
        for row in rows {
            let oid: u32 = row.get(0);
            let estimated: i64 = row.get(2);
            let kind: String = row.get(3);
            let relation_kind = match kind.as_str() {
                "v" => "view",
                "m" => "materialized view",
                "p" => "partitioned table",
                "f" => "foreign table",
                _ => "table",
            };
            let reason = if !row.get::<_, bool>(5) {
                Some(
                    "This account does not have table SELECT permission. Structure is available."
                        .into(),
                )
            } else if !matches!(kind.as_str(), "r" | "m") {
                Some(format!(
                    "This {relation_kind} is available for structure inspection. Executing view definitions, foreign scans and partition trees is not enabled."
                ))
            } else if row.get::<_, bool>(4) {
                Some("Row-level security policies may execute custom expressions. Structure is available; reading is not enabled.".into())
            } else if row.get::<_, bool>(6) {
                Some("This table has inherited children. Structure is available; reading the inheritance tree is not enabled.".into())
            } else {
                blocked.remove(&oid)
            };
            let mut columns = by_table.remove(&oid).unwrap_or_default();
            if reason.is_some() || kind != "r" {
                for column in &mut columns {
                    column.editable = false;
                    column.insertable = false;
                }
            }
            let deletable = self.manual_edits
                && kind == "r"
                && reason.is_none()
                && row.get::<_, bool>(7)
                && columns.iter().any(|c| c.primary_key);
            tables.push(TableInfo {
                insertable: self.manual_edits
                    && kind == "r"
                    && reason.is_none()
                    && row.get::<_, bool>(8)
                    && columns.iter().any(|c| c.primary_key),
                deletable,
                schema: schema.into(),
                name: row.get(1),
                columns,
                estimated_rows: (estimated >= 0).then_some(estimated),
                relation_kind: relation_kind.into(),
                read_blocked_reason: reason,
            });
        }
        Ok(tables)
    }

    async fn cancel(&self) -> bool {
        self.reusable.store(false, Ordering::Relaxed);
        let token = self.client.cancel_token();
        let result = match self.tls {
            TlsMode::LocalOnly => timeout(Duration::from_secs(2), token.cancel_query(NoTls)).await,
            TlsMode::VerifyFull | TlsMode::Require => {
                let Ok(tls) = tls_connector(self.tls, &self.root_ca) else {
                    return false;
                };
                timeout(
                    Duration::from_secs(2),
                    token.cancel_query(MakeTlsConnector::new(tls)),
                )
                .await
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
                timings: None,
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
                    "Cancellation requested; the connection was closed."
                } else { "The connection was closed. Server cancellation could not be confirmed; the server timeout still applies." }))
            },
            _ = tokio::time::sleep(Duration::from_secs(12)) => {
                let _ = self.cancel().await;
                Err(deadline())
            },
        }
    }

    fn browse_statement(
        table: &TableInfo,
        request: &BrowseRequest,
        count: bool,
    ) -> AppResult<(String, Vec<String>)> {
        let page_size = request.checked_page_size()?;
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
        let projections = if count {
            "COUNT(*)::pg_catalog.text".to_string()
        } else {
            projections
        };
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
                    parameters.push(format!(
                        "%{}%",
                        filter
                            .value
                            .replace('\\', "\\\\")
                            .replace('%', "\\%")
                            .replace('_', "\\_")
                    ));
                    format!(
                        "{name}::pg_catalog.text ILIKE ${}::pg_catalog.text",
                        parameters.len()
                    )
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
                    if text_comparison(column) {
                        format!(
                            "{name}::pg_catalog.text {op} ${}::pg_catalog.text",
                            parameters.len()
                        )
                    } else {
                        format!(
                            "{name} {op} ${}::pg_catalog.text::{}",
                            parameters.len(),
                            column_type_sql(column)
                        )
                    }
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
        if count {
            return Ok((sql, parameters));
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
                    .map(|name| {
                        let column = find_column(name).expect("sort columns were validated");
                        format!(
                            "{}.{}{} {} NULLS LAST",
                            quote_identifier(&table.name),
                            quote_identifier(name),
                            if text_comparison(column) {
                                "::pg_catalog.text"
                            } else {
                                ""
                            },
                            if request.descending { "DESC" } else { "ASC" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        sql.push_str(&format!(
            " LIMIT {} OFFSET {}",
            page_size + 1,
            request.page as usize * page_size
        ));
        Ok((sql, parameters))
    }

    pub async fn count(&self, table: &TableInfo, request: &BrowseRequest) -> AppResult<RowCount> {
        self.prevent_reuse();
        let (sql, parameters) = Self::browse_statement(table, request, true)?;
        // An exact count can scan the table: opt-in, time-bounded, never part of page loading.
        self.client
            .batch_execute("BEGIN READ ONLY; SET LOCAL statement_timeout = '5s'")
            .await
            .map_err(query_error)?;
        let params: Vec<(&(dyn tokio_postgres::types::ToSql + Sync), Type)> = parameters
            .iter()
            .map(|p| (p as &(dyn tokio_postgres::types::ToSql + Sync), Type::TEXT))
            .collect();
        let started = Instant::now();
        let row = self
            .client
            .query_typed_one(&sql, &params)
            .await
            .map_err(query_error)?;
        Ok(RowCount {
            rows: row.get(0),
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub async fn browse(
        &self,
        table: &TableInfo,
        request: &BrowseRequest,
    ) -> AppResult<QueryResult> {
        let page_size = request.checked_page_size()?;
        let (sql, parameters) = Self::browse_statement(table, request, false)?;
        let params: Vec<(&(dyn tokio_postgres::types::ToSql + Sync), Type)> = parameters
            .iter()
            .map(|p| (p as &(dyn tokio_postgres::types::ToSql + Sync), Type::TEXT))
            .collect();
        let started = Instant::now();
        let stream = self
            .client
            .query_typed_raw(&sql, params)
            .await
            .map_err(query_error)?;
        pin_mut!(stream);
        let mut result = QueryResult {
            columns: table.columns.iter().map(|c| c.name.clone()).collect(),
            rows: vec![],
            elapsed_ms: 0,
            timings: None,
            truncated: false,
            sql,
        };
        let mut bytes = 0;
        while let Some(row) = stream.try_next().await.map_err(query_error)? {
            if result.rows.len() == page_size {
                // LIMIT page_size + 1 already bounds this read. The extra row
                // only signals another page; don't open a separate cancellation
                // connection (and TLS handshake) for an already bounded query.
                result.truncated = true;
                continue; // Consume command completion before reusing this transport.
            }
            let values = (0..row.len())
                .map(|i| row.try_get::<_, Option<String>>(i))
                .collect::<Result<Vec<_>, _>>()
                .map_err(query_error)?;
            bytes += values
                .iter()
                .map(|v| v.as_ref().map_or(4, String::len))
                .sum::<usize>();
            if bytes > MAX_RESULT_BYTES {
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
        self.client.query_typed(
            "SELECT ci.relname, pg_get_indexdef(i.indexrelid), COALESCE(s.idx_scan,0)::text,
             pg_size_pretty(pg_relation_size(ci.oid)), i.indisunique, i.indisprimary
             FROM pg_index i JOIN pg_class ct ON ct.oid = i.indrelid
             JOIN pg_namespace n ON n.oid = ct.relnamespace JOIN pg_class ci ON ci.oid = i.indexrelid
             LEFT JOIN pg_stat_user_indexes s ON s.indexrelid = i.indexrelid
             WHERE n.nspname = $1 AND ct.relname = $2 AND has_table_privilege(ct.oid, 'SELECT')
             ORDER BY ci.relname LIMIT 100", &[(&schema, Type::TEXT), (&table, Type::TEXT)]).await.map_err(query_error)
            .map(|rows| rows.into_iter().map(|r| IndexInfo { name: r.get(0), definition: r.get(1), scans: r.get(2), size: r.get(3), unique: r.get(4), primary: r.get(5) }).collect())
    }

    pub async fn overview(&self) -> AppResult<Overview> {
        let row = self.client.query_typed_one(
            "SELECT current_database(), current_setting('server_version'), pg_size_pretty(pg_database_size(current_database())),
             numbackends::bigint, (SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND state = 'active'),
             xact_commit::text, xact_rollback::text,
             CASE WHEN blks_hit + blks_read > 0 THEN 100.0 * blks_hit::float8 / (blks_hit::float8 + blks_read::float8) ELSE NULL END,
             stats_reset::text, clock_timestamp()::text, pg_database_size(current_database())::text FROM pg_stat_database WHERE datname = current_database()", &[])
            .await.map_err(query_error)?;
        Ok(Overview {
            database: row.get(0),
            server_version: row.get(1),
            database_size: row.get(2),
            database_size_bytes: row.get(10),
            connections: row.get(3),
            active_connections: row.get(4),
            transactions_committed: row.get(5),
            transactions_rolled_back: row.get(6),
            cache_hit_percent: row.get(7),
            stats_reset: row.get(8),
            observed_at: row.get(9),
        })
    }

    pub async fn approve_query(&self, sql: &str, schema: &str) -> AppResult<policy::ApprovedQuery> {
        let relation = policy::postgres_relation(sql)?;
        let metadata = async {
            match relation {
                Some((ref scope, ref name)) if scope == schema => {
                    Ok(vec![self.approved_table(scope, name).await?])
                }
                Some(_) => Err(AppError::policy(
                    "This table is outside the selected schema.",
                )),
                None => Ok(Vec::new()),
            }
        };
        let (_, tables) = tokio::try_join!(self.preflight(), metadata)?;
        policy::approve(sql, &tables)
    }

    pub async fn stop(&self) {
        let _ = self.cancel().await;
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    #[test]
    fn supported_and_future_versions_are_not_blocked() {
        for version in [
            "150000", "150018", "160000", "170007", "180000", "190000", "200001",
        ] {
            assert!(validate_server_version(version).is_ok(), "{version}");
        }
        for version in ["140999", "90624", "0", "", "unknown"] {
            assert_eq!(
                validate_server_version(version).unwrap_err().code,
                "UNSUPPORTED_VERSION"
            );
        }
    }

    fn certificate() -> (String, String) {
        let key = rcgen::KeyPair::from_pem(include_str!("../tests/fixtures/tls/test-only-key.pem"))
            .unwrap();
        let mut params = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap();
        let now = std::time::SystemTime::now();
        params.not_before = (now - Duration::from_secs(60)).into();
        params.not_after = (now + Duration::from_secs(86400)).into();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        (params.self_signed(&key).unwrap().pem(), key.serialize_pem())
    }

    fn request(port: u16, tls: TlsMode) -> ConnectRequest {
        ConnectRequest {
            engine: Engine::Postgres,
            name: "TLS fixture".into(),
            host: "127.0.0.1".into(),
            port,
            database: "fixture".into(),
            username: "reader".into(),
            password: "CANARY_MUST_NOT_LEAK".into(),
            schema: "public".into(),
            tls,
            root_ca_pem: String::new(),
            manual_edits: false,
        }
    }

    async fn handshake(mode: TlsMode, trust_ca: bool, hostname: &str) -> bool {
        let (cert, key) = certificate();
        let identity = native_tls::Identity::from_pkcs8(cert.as_bytes(), key.as_bytes()).unwrap();
        let acceptor =
            tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let _ = acceptor.accept(socket).await;
        });
        let certificates = if trust_ca {
            parse_root_ca(&cert).unwrap()
        } else {
            vec![]
        };
        let connector =
            tokio_native_tls::TlsConnector::from(tls_connector(mode, &certificates).unwrap());
        let result = connector
            .connect(hostname, TcpStream::connect(address).await.unwrap())
            .await;
        let connected = result.is_ok();
        server.await.unwrap();
        connected
    }

    #[tokio::test]
    async fn tls_verification_and_explicit_compatibility_are_distinct() {
        assert!(!handshake(TlsMode::VerifyFull, false, "localhost").await);
        assert!(handshake(TlsMode::VerifyFull, true, "localhost").await);
        assert!(!handshake(TlsMode::VerifyFull, true, "127.0.0.1").await);
        assert!(handshake(TlsMode::Require, false, "127.0.0.1").await);
    }

    #[tokio::test]
    async fn require_tls_never_falls_back_or_sends_credentials_to_plaintext_server() {
        for mode in [TlsMode::VerifyFull, TlsMode::Require] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut ssl_request = [0; 8];
                socket.read_exact(&mut ssl_request).await.unwrap();
                assert_eq!(ssl_request, [0, 0, 0, 8, 4, 210, 22, 47]);
                socket.write_all(b"N").await.unwrap();
                let mut buffer = [0; 128];
                assert_eq!(
                    timeout(Duration::from_secs(2), socket.read(&mut buffer))
                        .await
                        .unwrap()
                        .unwrap(),
                    0
                );
            });
            let error = Profile::new(request(port, mode))
                .unwrap()
                .open()
                .await
                .err()
                .unwrap();
            assert_eq!(error.code, "TLS_CONNECTION_FAILED");
            assert!(error.message.contains("does not support TLS"));
            assert!(!error.message.contains("CANARY"));
            server.await.unwrap();
        }
    }

    #[test]
    fn ca_input_rejects_keys_garbage_and_oversize_bundles() {
        let (cert, key) = certificate();
        assert_eq!(parse_root_ca(&cert).unwrap().len(), 1);
        for invalid in [
            key,
            "x".repeat(65537),
            format!("{cert}garbage"),
            cert.repeat(17),
        ] {
            assert_eq!(parse_root_ca(&invalid).err().unwrap().code, "INVALID_CA");
        }
        let mut r = request(5432, TlsMode::Require);
        r.root_ca_pem = cert;
        assert_eq!(Profile::new(r).err().unwrap().code, "INVALID_CA");
    }

    #[tokio::test]
    async fn network_failure_has_actionable_sanitized_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let error = Profile::new(request(port, TlsMode::VerifyFull))
            .unwrap()
            .open()
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, "NETWORK_ERROR");
        assert!(!error.message.contains("CANARY"));
    }

    #[tokio::test]
    #[ignore = "requires the disposable PostgreSQL Compose fixture"]
    async fn encrypted_connection_authenticates_and_queries_through_tls() {
        let (cert, key) = certificate();
        let identity = native_tls::Identity::from_pkcs8(cert.as_bytes(), key.as_bytes()).unwrap();
        let acceptor =
            tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let proxy = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 8];
            socket.read_exact(&mut request).await.unwrap();
            assert_eq!(request, [0, 0, 0, 8, 4, 210, 22, 47]);
            socket.write_all(b"S").await.unwrap();
            let mut tls = acceptor.accept(socket).await.unwrap();
            let fixture_port = std::env::var("BUGGY_TEST_POSTGRES_PORT")
                .unwrap_or_else(|_| "15432".into())
                .parse::<u16>()
                .unwrap();
            let mut fixture = TcpStream::connect(("127.0.0.1", fixture_port))
                .await
                .unwrap();
            let _ = tokio::io::copy_bidirectional(&mut tls, &mut fixture).await;
        });
        let mut r = request(port, TlsMode::VerifyFull);
        r.host = "localhost".into();
        r.root_ca_pem = cert;
        r.database = "tablepor_test".into();
        r.username = "tablepor_reader".into();
        r.password = "local-reader-only".into();
        let profile = Profile::new(r).unwrap();
        let db = profile.open().await.unwrap();
        assert_eq!(
            db.client
                .query_one("SELECT 1::int4", &[])
                .await
                .unwrap()
                .get::<_, i32>(0),
            1
        );
        assert_eq!(
            db.client
                .query_one("SHOW transaction_read_only", &[])
                .await
                .unwrap()
                .get::<_, String>(0),
            "on"
        );
        profile.recycle(db);
        let reused = profile.open().await.unwrap();
        assert!(
            reused.reused,
            "the TLS proxy accepts exactly one physical connection"
        );
        assert_eq!(
            reused
                .client
                .query_typed_one("SELECT 2::int4", &[])
                .await
                .unwrap()
                .get::<_, i32>(0),
            2
        );
        assert_eq!(
            reused
                .client
                .execute("DELETE FROM public.orders WHERE false", &[])
                .await
                .unwrap_err()
                .code(),
            Some(&tokio_postgres::error::SqlState::READ_ONLY_SQL_TRANSACTION)
        );
        drop(reused);
        proxy.abort();
    }
}
