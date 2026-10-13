//! Small application layer: session ownership, job limits and cancellation.
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tokio::sync::{RwLock, watch};
use uuid::Uuid;

use crate::{models::*, mysql, postgres::Profile, redis_store, remote_profile::RemoteProfile};

enum Session {
    Postgres(Profile),
    Mysql(RemoteProfile),
    Redis(RemoteProfile),
}
impl Session {
    fn postgres(&self) -> AppResult<&Profile> {
        match self {
            Self::Postgres(p) => Ok(p),
            _ => Err(AppError::policy(
                "This operation is unavailable for this engine.",
            )),
        }
    }
}

struct Job {
    connection_id: String,
    cancel: watch::Sender<bool>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

#[derive(Default)]
pub struct Workbench {
    sessions: RwLock<HashMap<String, Arc<Session>>>,
    jobs: Jobs,
    updating: AtomicBool,
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

pub struct UpdateGuard<'a>(&'a Workbench);
impl Drop for UpdateGuard<'_> {
    fn drop(&mut self) {
        self.0.updating.store(false, Ordering::SeqCst);
    }
}

impl Workbench {
    /// Share the job lock so a query cannot start between checking and locking updates.
    pub fn begin_update(&self) -> AppResult<UpdateGuard<'_>> {
        let jobs = self.jobs.lock().map_err(|_| internal_error())?;
        if !jobs.is_empty() || self.updating.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                "BUSY",
                "Finish database operations before installing an update.",
            ));
        }
        Ok(UpdateGuard(self))
    }

    pub async fn connect(&self, request: ConnectRequest) -> AppResult<ConnectionInfo> {
        let id = Uuid::new_v4().to_string();
        let (_connection_job, _) = self.register_job(&id, &id)?;
        let profile = Arc::new(match request.engine {
            Engine::Postgres => Session::Postgres(Profile::new(request)?),
            Engine::Mysql => Session::Mysql(RemoteProfile::new(request)?),
            Engine::Redis => Session::Redis(RemoteProfile::new(request)?),
        });
        let info = tokio::time::timeout(Duration::from_secs(20), async {
            match profile.as_ref() {
                Session::Postgres(p) => {
                    let db = p.open().await?;
                    let version = db.preflight().await?;
                    let tables = db.tables(&p.schema).await?;
                    Ok(p.info(id.clone(), version, tables))
                }
                Session::Mysql(p) => mysql::connect(p, id.clone()).await,
                Session::Redis(p) => redis_store::connect(p, id.clone()).await,
            }
        })
        .await
        .map_err(|_| AppError::new("TIMEOUT", "Connection preflight timed out."))??;
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
    ) -> AppResult<(Arc<Session>, JobGuard, watch::Receiver<bool>)> {
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
        if self.updating.load(Ordering::SeqCst) {
            return Err(AppError::new(
                "UPDATING",
                "Wait for the application update to finish.",
            ));
        }
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
        let profile = profile.postgres()?;
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
        if let Session::Mysql(p) = profile.as_ref() {
            return bounded_metadata(
                async {
                    match &operation {
                        Operation::Sql(r) => mysql::query(p, r).await,
                        Operation::Browse(r) => mysql::browse(p, r).await,
                    }
                },
                cancelled,
            )
            .await;
        }
        let profile = profile.postgres()?;
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
        if let Session::Mysql(p) = profile.as_ref() {
            return bounded_metadata(mysql::indexes(p, table_name), cancelled).await;
        }
        let profile = profile.postgres()?;
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

    pub async fn redis_command(&self, request: RedisRequest) -> AppResult<RedisResult> {
        let (profile, _guard, cancelled) = self
            .start_operation(&request.connection_id, &request.request_id)
            .await?;
        let Session::Redis(p) = profile.as_ref() else {
            return Err(AppError::policy(
                "This command requires a Redis connection.",
            ));
        };
        bounded_metadata(redis_store::command(p, &request.args), cancelled).await
    }

    pub async fn overview(&self, connection_id: &str) -> AppResult<Overview> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        if let Session::Mysql(p) = profile.as_ref() {
            return bounded_metadata(mysql::overview(p), cancelled).await;
        }
        let profile = profile.postgres()?;
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

#[cfg(test)]
mod update_tests {
    use super::*;
    #[test]
    fn update_and_database_jobs_are_mutually_exclusive() {
        let workbench = Workbench::default();
        let id = Uuid::new_v4().to_string();
        let (job, _) = workbench.register_job("session", &id).unwrap();
        assert!(workbench.begin_update().is_err());
        drop(job);
        let update = workbench.begin_update().unwrap();
        assert!(workbench.begin_update().is_err());
        assert!(workbench.register_job("session", &id).is_err());
        drop(update);
        assert!(workbench.register_job("session", &id).is_ok());
    }
}
