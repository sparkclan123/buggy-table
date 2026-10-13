//! Small application layer: session ownership, job limits and cancellation.
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::{RwLock, watch};
use uuid::Uuid;

use crate::{models::*, postgres::Profile};

struct Job {
    connection_id: String,
    cancel: watch::Sender<bool>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

#[derive(Default)]
pub struct Workbench {
    sessions: RwLock<HashMap<String, Arc<Profile>>>,
    jobs: Jobs,
}

struct JobGuard {
    id: String,
    jobs: Jobs,
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(&self.id);
        }
    }
}

enum Operation {
    Sql(QueryRequest),
    Browse(BrowseRequest),
}

impl Workbench {
    pub async fn connect(&self, request: ConnectRequest) -> AppResult<ConnectionInfo> {
        let profile = Arc::new(Profile::new(request)?);
        let result = tokio::time::timeout(Duration::from_secs(20), async {
            let db = profile.open().await?;
            let version = db.preflight().await?;
            let tables = db.tables(&profile.schema).await?;
            Ok::<_, AppError>((version, tables))
        })
        .await
        .map_err(|_| AppError::new("TIMEOUT", "Connection preflight timed out."))??;
        let id = Uuid::new_v4().to_string();
        let info = profile.info(id.clone(), result.0, result.1);
        let mut sessions = self.sessions.write().await;
        if sessions.len() >= 8 {
            return Err(AppError::new(
                "SESSION_LIMIT",
                "Disconnect an existing session before adding another (maximum 8).",
            ));
        }
        sessions.insert(id, profile);
        Ok(info)
    }

    pub async fn disconnect(&self, connection_id: &str) -> AppResult<()> {
        self.sessions.write().await.remove(connection_id);
        let jobs = self.jobs.lock().map_err(|_| internal_error())?;
        for job in jobs.values().filter(|j| j.connection_id == connection_id) {
            let _ = job.cancel.send(true);
        }
        Ok(())
    }

    async fn start_operation(
        &self,
        id: &str,
        request_id: &str,
    ) -> AppResult<(Arc<Profile>, JobGuard, watch::Receiver<bool>)> {
        // Keep the session read lock until the job is registered. Disconnect
        // cannot miss a job between cloning its profile and registering it.
        let sessions = self.sessions.read().await;
        let profile = sessions.get(id).cloned().ok_or_else(|| {
            AppError::new("NOT_CONNECTED", "This session has ended. Connect again.")
        })?;
        let (guard, cancelled) = self.register_job(id, request_id)?;
        Ok((profile, guard, cancelled))
    }

    fn register_job(
        &self,
        connection_id: &str,
        request_id: &str,
    ) -> AppResult<(JobGuard, watch::Receiver<bool>)> {
        if Uuid::parse_str(request_id).is_err() {
            return Err(AppError::new(
                "INVALID_REQUEST",
                "A valid request UUID is required.",
            ));
        }
        let mut jobs = self.jobs.lock().map_err(|_| internal_error())?;
        if jobs.contains_key(request_id) || jobs.values().any(|j| j.connection_id == connection_id)
        {
            return Err(AppError::new(
                "BUSY",
                "Wait for or cancel the active query on this connection.",
            ));
        }
        let (sender, receiver) = watch::channel(false);
        jobs.insert(
            request_id.into(),
            Job {
                connection_id: connection_id.into(),
                cancel: sender,
            },
        );
        Ok((
            JobGuard {
                id: request_id.into(),
                jobs: self.jobs.clone(),
            },
            receiver,
        ))
    }

    pub fn cancel(&self, connection_id: &str, request_id: &str) -> AppResult<()> {
        let jobs = self.jobs.lock().map_err(|_| internal_error())?;
        if let Some(job) = jobs
            .get(request_id)
            .filter(|j| j.connection_id == connection_id)
        {
            let _ = job.cancel.send(true);
        }
        Ok(())
    }

    pub async fn query(&self, request: QueryRequest) -> AppResult<QueryResult> {
        self.run(Operation::Sql(request)).await
    }

    pub async fn browse(&self, request: BrowseRequest) -> AppResult<QueryResult> {
        self.run(Operation::Browse(request)).await
    }

    pub async fn save(&self, request: SaveRequest) -> AppResult<SaveResult> {
        let (profile, _guard, mut cancelled) = self
            .start_operation(&request.connection_id, &request.request_id)
            .await?;
        if !profile.manual_edits {
            return Err(AppError::policy(
                "Manual editing is disabled for this connection.",
            ));
        }
        let work = async {
            let db = profile.open().await?;
            db.save(&profile.schema, &request).await
        };
        // A lost response during COMMIT cannot prove rollback. Never retry a save
        // automatically; the UI requires a fresh read to reconcile this outcome.
        tokio::select! {
            result = work => result,
            _ = async { let _ = cancelled.wait_for(|v| *v).await; } => Err(save_uncertain()),
            _ = tokio::time::sleep(Duration::from_secs(20)) => Err(save_uncertain()),
        }
    }

    async fn run(&self, operation: Operation) -> AppResult<QueryResult> {
        let (connection_id, request_id) = match &operation {
            Operation::Sql(r) => (&r.connection_id, &r.request_id),
            Operation::Browse(r) => (&r.connection_id, &r.request_id),
        };
        let (profile, _guard, mut cancelled) =
            self.start_operation(connection_id, request_id).await?;
        let db = tokio::select! {
            result = profile.open() => result?,
            _ = async { let _ = cancelled.wait_for(|value| *value).await; } => return Err(AppError::new("CANCELLED", "Connection attempt cancelled.")),
        };
        let work = async {
            match &operation {
                Operation::Sql(r) => {
                    db.run_validated(&r.sql, &profile.schema, r.explain, cancelled.clone())
                        .await
                }
                Operation::Browse(r) => {
                    db.preflight().await?;
                    let tables = db.tables(&profile.schema).await?;
                    let table = tables.iter().find(|t| t.name == r.table)
                        .ok_or_else(|| AppError::policy("This table is no longer in the approved scope. Reconnect to refresh the schema."))?;
                    db.browse(table, r).await
                }
            }
        };
        let mut stop = cancelled.clone();
        tokio::select! {
            result = work => result,
            _ = async { let _ = stop.wait_for(|value| *value).await; } => {
                db.stop().await;
                Err(AppError::new("CANCELLED", "Cancellation requested. This operation's connection has been closed."))
            },
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                db.stop().await;
                Err(AppError::new("TIMEOUT", "Operation exceeded 15 seconds; cancellation requested and connection closed."))
            },
        }
    }

    pub async fn indexes(
        &self,
        connection_id: &str,
        table_name: &str,
    ) -> AppResult<Vec<IndexInfo>> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                db.preflight().await?;
                let tables = db.tables(&profile.schema).await?;
                if !tables.iter().any(|t| t.name == table_name) {
                    return Err(AppError::policy("Table is outside the approved scope."));
                }
                db.indexes(&profile.schema, table_name).await
            },
            cancelled,
        )
        .await
    }

    pub async fn overview(&self, connection_id: &str) -> AppResult<Overview> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                db.preflight().await?;
                db.overview().await
            },
            cancelled,
        )
        .await
    }
}

async fn bounded_metadata<T>(
    work: impl Future<Output = AppResult<T>>,
    mut cancelled: watch::Receiver<bool>,
) -> AppResult<T> {
    tokio::select! {
        result = work => result,
        _ = async { let _ = cancelled.wait_for(|value| *value).await; } => Err(AppError::new("CANCELLED", "Metadata connection closed after disconnect.")),
        _ = tokio::time::sleep(Duration::from_secs(15)) => Err(AppError::new("TIMEOUT", "Metadata operation timed out and its connection was closed.")),
    }
}

fn internal_error() -> AppError {
    AppError::new(
        "INTERNAL_ERROR",
        "Application state is unavailable. Restart the app.",
    )
}

fn save_uncertain() -> AppError {
    AppError::new(
        "SAVE_UNCERTAIN",
        "The save response was interrupted. Reload this table to check the database before making further edits.",
    )
}
