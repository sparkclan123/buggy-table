use serde::{Deserialize, Serialize};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn policy(message: impl Into<String>) -> Self {
        Self::new("POLICY_DENIED", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

// Intentionally not Debug or Serialize: never return or log connection secrets.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    #[serde(default)]
    pub engine: Engine,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub schema: String,
    pub tls: TlsMode,
    #[serde(default)]
    pub manual_edits: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    #[default]
    Postgres,
    Mysql,
    Redis,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TlsMode {
    VerifyFull,
    LocalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    #[serde(default)]
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub estimated_rows: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub engine: Engine,
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub schema: String,
    pub tls: TlsMode,
    pub server_version: String,
    pub manual_edits: bool,
    pub tables: Vec<TableInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRequest {
    pub connection_id: String,
    pub request_id: String,
    pub sql: String,
    #[serde(default)]
    pub explain: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    // Text preserves PostgreSQL decimals, bigint and timestamps without JS rounding.
    pub rows: Vec<Vec<Option<String>>>,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub sql: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    pub column: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterMatch {
    #[default]
    All,
    Any,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseRequest {
    pub connection_id: String,
    pub request_id: String,
    pub table: String,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub filter_match: FilterMatch,
    pub sort_column: Option<String>,
    pub descending: bool,
    pub page: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub definition: String,
    pub scans: String,
    pub size: String,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub database: String,
    pub server_version: String,
    pub database_size: String,
    pub connections: i64,
    pub active_connections: i64,
    pub transactions_committed: String,
    pub transactions_rolled_back: String,
    pub cache_hit_percent: Option<f64>,
    pub stats_reset: Option<String>,
    pub observed_at: String,
}

// Manual grid saves never accept SQL. AI/query execution does not call this API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRequest {
    pub connection_id: String,
    pub request_id: String,
    pub table: String,
    pub rows: Vec<RowChange>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RowChange {
    pub original: std::collections::BTreeMap<String, Option<String>>,
    pub changes: std::collections::BTreeMap<String, Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub updated_rows: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedisRequest {
    pub connection_id: String,
    pub request_id: String,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisResult {
    pub value: serde_json::Value,
    pub elapsed_ms: u64,
}
