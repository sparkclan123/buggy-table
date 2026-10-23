use crate::{models::*, service::Workbench};
use tauri::State;

#[tauri::command]
pub async fn connect(
    state: State<'_, Workbench>,
    request: ConnectRequest,
) -> AppResult<ConnectionInfo> {
    state.connect(request).await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, Workbench>, connection_id: String) -> AppResult<()> {
    state.disconnect(&connection_id).await
}

#[tauri::command]
pub async fn run_query(
    state: State<'_, Workbench>,
    request: QueryRequest,
) -> AppResult<QueryResult> {
    state.query(request).await
}

#[tauri::command]
pub fn cancel_query(
    state: State<'_, Workbench>,
    connection_id: String,
    request_id: String,
) -> AppResult<()> {
    state.cancel(&connection_id, &request_id)
}

#[tauri::command]
pub async fn browse_table(
    state: State<'_, Workbench>,
    request: BrowseRequest,
) -> AppResult<QueryResult> {
    state.browse(request).await
}

#[tauri::command]
pub async fn inspect_table(
    state: State<'_, Workbench>,
    connection_id: String,
    table: String,
    schema: Option<String>,
    database: Option<String>,
) -> AppResult<Vec<IndexInfo>> {
    state
        .indexes_in_database(
            &connection_id,
            &table,
            schema.as_deref(),
            database.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn get_overview(
    state: State<'_, Workbench>,
    connection_id: String,
    database: Option<String>,
) -> AppResult<Overview> {
    state
        .overview_in_database(&connection_id, database.as_deref())
        .await
}

#[tauri::command]
pub async fn save_table_changes(
    state: State<'_, Workbench>,
    request: SaveRequest,
) -> AppResult<SaveResult> {
    state.save(request).await
}

#[tauri::command]
pub async fn redis_command(
    state: State<'_, Workbench>,
    request: RedisRequest,
) -> AppResult<RedisResult> {
    state.redis_command(request).await
}

#[tauri::command]
pub async fn count_table_rows(
    state: State<'_, Workbench>,
    request: BrowseRequest,
) -> AppResult<RowCount> {
    state.count(request).await
}
