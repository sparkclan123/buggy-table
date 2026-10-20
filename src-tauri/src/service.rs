//! Small application layer: session ownership, job limits and cancellation.
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::sync::{RwLock, watch};
use uuid::Uuid;

use crate::{
    models::*,
    mysql,
    policy::ApprovedQuery,
    postgres::{Database, Profile},
    redis_store,
    remote_profile::RemoteProfile,
};

enum Adapter {
    Postgres(Box<Profile>),
    Mysql(RemoteProfile),
    Redis(RemoteProfile),
}
struct Session {
    adapter: Adapter,
    request: ConnectRequest,
    password: zeroize::Zeroizing<String>,
    route: Option<Arc<crate::vpn::runtime::Tunnel>>,
    owner: Option<String>,
}
impl Session {
    fn postgres(&self) -> AppResult<&Profile> {
        match &self.adapter {
            Adapter::Postgres(p) => Ok(p),
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

enum PreparedRead {
    Sql(ApprovedQuery),
    Browse(TableInfo),
}

impl Operation {
    async fn prepare(&self, db: &Database, schema: &str) -> AppResult<PreparedRead> {
        match self {
            Self::Sql(request) => Ok(PreparedRead::Sql(
                db.approve_query(&request.sql, request.schema.as_deref().unwrap_or(schema))
                    .await?,
            )),
            Self::Browse(request) => {
                let (_, table) = tokio::try_join!(
                    db.preflight(),
                    db.approved_table(request.schema.as_deref().unwrap_or(schema), &request.table)
                )?;
                Ok(PreparedRead::Browse(table))
            }
        }
    }
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
        self.connect_route(request, None, None, None).await
    }

    pub async fn connect_route(
        &self,
        mut request: ConnectRequest,
        route: Option<Arc<crate::vpn::runtime::Tunnel>>,
        replacing: Option<&str>,
        owner: Option<String>,
    ) -> AppResult<ConnectionInfo> {
        let _old_job = if let Some(old) = replacing {
            Some(self.register_job(old, &Uuid::new_v4().to_string())?.0)
        } else {
            None
        };
        if let Some(route) = &route {
            route.ensure_alive()?;
        }
        let id = Uuid::new_v4().to_string();
        let (_connection_job, _) = self.register_job(&id, &id)?;
        let endpoint = route.as_ref().map(|r| r.endpoint);
        let adapter = match request.engine {
            Engine::Postgres => {
                let mut p = Profile::new(request.clone())?;
                p.endpoint = endpoint;
                Adapter::Postgres(Box::new(p))
            }
            Engine::Mysql => {
                let mut p = RemoteProfile::new(request.clone())?;
                p.endpoint = endpoint;
                Adapter::Mysql(p)
            }
            Engine::Redis => {
                let mut p = RemoteProfile::new(request.clone())?;
                p.endpoint = endpoint;
                Adapter::Redis(p)
            }
        };
        let password = zeroize::Zeroizing::new(std::mem::take(&mut request.password));
        let profile = Arc::new(Session {
            adapter,
            request,
            password,
            route,
            owner,
        });
        let mut info = tokio::time::timeout(Duration::from_secs(20), async {
            match &profile.adapter {
                Adapter::Postgres(p) => {
                    let db = p.open().await?;
                    let (version, tables) = tokio::try_join!(db.preflight(), db.tables(&p.schema))?;
                    p.recycle(db);
                    Ok(p.info(id.clone(), version, tables))
                }
                Adapter::Mysql(p) => mysql::connect(p, id.clone()).await,
                Adapter::Redis(p) => redis_store::connect(p, id.clone()).await,
            }
        })
        .await
        .map_err(|_| AppError::new("TIMEOUT", "Connection preflight timed out."))??;
        let mut sessions = self.sessions.write().await;
        info.vpn_owner = profile.owner.clone();
        if let Some(route) = &profile.route {
            route.ensure_alive()?;
            info.vpn = crate::vpn::VpnSelection {
                enabled: true,
                profile_id: Some(route.profile.id.clone()),
            };
        }
        if replacing.is_some_and(|old| !sessions.contains_key(old)) {
            return Err(AppError::new(
                "NOT_CONNECTED",
                "The session ended while reconnecting.",
            ));
        }
        if sessions.len() >= 8 && replacing.is_none() {
            return Err(AppError::new(
                "SESSION_LIMIT",
                "Disconnect an existing session before adding another (maximum 8).",
            ));
        }
        if let Some(old) = replacing {
            sessions.remove(old);
        }
        sessions.insert(id, profile);
        Ok(info)
    }

    pub async fn connection_request(&self, id: &str) -> AppResult<ConnectRequest> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(id)
            .ok_or_else(|| AppError::new("NOT_CONNECTED", "This session has ended."))?;
        let mut request = session.request.clone();
        request.password = session.password.to_string();
        Ok(request)
    }
    pub async fn check_owner(&self, id: &str, owner: &str) -> AppResult<()> {
        if !self
            .sessions
            .read()
            .await
            .get(id)
            .is_some_and(|s| s.owner.as_deref() == Some(owner))
        {
            return Err(AppError::new(
                "VPN_PROFILE",
                "The VPN settings belong to a different connection.",
            ));
        }
        Ok(())
    }
    pub async fn vpn_profile_in_use(&self, id: &str) -> bool {
        self.sessions
            .read()
            .await
            .values()
            .any(|s| s.route.as_ref().is_some_and(|r| r.profile.id == id))
    }
    pub async fn vpn_owner_in_use(&self, owner: &str) -> bool {
        self.sessions
            .read()
            .await
            .values()
            .any(|s| s.owner.as_deref() == Some(owner))
    }
    pub async fn vpn_status(
        &self,
        id: &str,
        probe: bool,
    ) -> AppResult<crate::vpn::runtime::VpnStatus> {
        let route = self
            .sessions
            .read()
            .await
            .get(id)
            .ok_or_else(|| AppError::new("NOT_CONNECTED", "This session has ended."))?
            .route
            .clone();
        if let Some(route) = route {
            if probe {
                let _ = route.probe().await;
            }
            Ok(route.status())
        } else {
            Ok(crate::vpn::runtime::VpnStatus::direct())
        }
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
        if let Some(route) = &profile.route {
            route.ensure_alive()?;
        }
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

    pub async fn count(&self, request: BrowseRequest) -> AppResult<RowCount> {
        let started = Instant::now();
        let (profile, _guard, cancelled) = self
            .start_operation(&request.connection_id, &request.request_id)
            .await?;
        let mut result = bounded_metadata(
            async {
                if let Adapter::Mysql(p) = &profile.adapter {
                    return mysql::count(p, &request).await;
                }
                let profile = profile.postgres()?;
                // SET LOCAL count timeout is isolated from reusable read sessions.
                let db = profile.open_dedicated().await?;
                let (_, table) = tokio::try_join!(
                    db.preflight(),
                    db.approved_table(
                        request.schema.as_deref().unwrap_or(&profile.schema),
                        &request.table
                    )
                )?;
                db.count(&table, &request).await
            },
            cancelled,
        )
        .await?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
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
            // Writes can execute triggers that change session state. Never pool them.
            let db = profile.open_dedicated().await?;
            db.save(
                request.schema.as_deref().unwrap_or(&profile.schema),
                &request,
            )
            .await
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
        let started = Instant::now();
        let (connection_id, request_id) = match &operation {
            Operation::Sql(r) => (&r.connection_id, &r.request_id),
            Operation::Browse(r) => (&r.connection_id, &r.request_id),
        };
        let (profile, _guard, mut cancelled) =
            self.start_operation(connection_id, request_id).await?;
        if let Adapter::Mysql(p) = &profile.adapter {
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
        let connecting = Instant::now();
        let mut db = tokio::select! {
            result = profile.open() => result?,
            _ = async { let _ = cancelled.wait_for(|value| *value).await; } => return Err(AppError::new("CANCELLED", "Connection attempt cancelled.")),
        };
        let mut connection_ms = connecting.elapsed().as_millis() as u64;
        let operation_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let validating = Instant::now();
        let mut prepared = bounded_metadata_until(
            operation.prepare(&db, &profile.schema),
            cancelled.clone(),
            operation_deadline,
        )
        .await;
        let mut validation_ms = validating.elapsed().as_millis() as u64;
        // Only fixed catalog checks may be retried, and only on a dead reused
        // transport. No user SELECT or save has been sent at this point.
        if db.reused
            && prepared
                .as_ref()
                .is_err_and(|e| e.code == "CONNECTION_LOST")
        {
            drop(db);
            let reconnecting = Instant::now();
            db = bounded_metadata_until(profile.open(), cancelled.clone(), operation_deadline)
                .await?;
            connection_ms += reconnecting.elapsed().as_millis() as u64;
            let validating = Instant::now();
            prepared = bounded_metadata_until(
                operation.prepare(&db, &profile.schema),
                cancelled.clone(),
                operation_deadline,
            )
            .await;
            validation_ms += validating.elapsed().as_millis() as u64;
        }
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                if matches!(error.code, "CANCELLED" | "TIMEOUT") {
                    db.stop().await;
                }
                return Err(error);
            }
        };
        let work = async {
            match (&operation, prepared) {
                (Operation::Sql(r), PreparedRead::Sql(sql)) => {
                    db.execute(sql.sql(), r.explain, cancelled.clone()).await
                }
                (Operation::Browse(r), PreparedRead::Browse(table)) => db.browse(&table, r).await,
                _ => Err(internal_error()),
            }
        };
        let mut stop = cancelled.clone();
        let mut result = tokio::select! {
            result = work => result,
            _ = async { let _ = stop.wait_for(|value| *value).await; } => {
                db.stop().await;
                Err(AppError::new("CANCELLED", "Cancellation requested. This operation's connection has been closed."))
            },
            _ = tokio::time::sleep_until(operation_deadline) => {
                db.stop().await;
                Err(AppError::new("TIMEOUT", "Operation exceeded 15 seconds; cancellation requested and connection closed."))
            },
        }?;
        let connection_reused = db.reused;
        let cleanup = Instant::now();
        profile.recycle(db);
        let cleanup_ms = cleanup.elapsed().as_millis() as u64;
        let execution_ms = result.elapsed_ms;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        result.timings = Some(QueryTimings {
            connection_ms,
            connection_reused,
            validation_ms,
            execution_ms,
            cleanup_ms,
        });
        Ok(result)
    }

    pub async fn indexes(
        &self,
        connection_id: &str,
        table_name: &str,
    ) -> AppResult<Vec<IndexInfo>> {
        self.indexes_in_schema(connection_id, table_name, None)
            .await
    }

    pub async fn indexes_in_schema(
        &self,
        connection_id: &str,
        table_name: &str,
        schema: Option<&str>,
    ) -> AppResult<Vec<IndexInfo>> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        if let Adapter::Mysql(p) = &profile.adapter {
            return bounded_metadata(mysql::indexes(p, table_name), cancelled).await;
        }
        let profile = profile.postgres()?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                let schema = schema.unwrap_or(&profile.schema);
                tokio::try_join!(db.preflight(), db.approved_table(schema, table_name))?;
                let result = db.indexes(schema, table_name).await?;
                profile.recycle(db);
                Ok(result)
            },
            cancelled,
        )
        .await
    }

    pub async fn schemas(&self, connection_id: &str) -> AppResult<Vec<String>> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        let profile = profile.postgres()?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                let (_, result) = tokio::try_join!(db.preflight(), db.schemas())?;
                profile.recycle(db);
                Ok(result)
            },
            cancelled,
        )
        .await
    }

    pub async fn schema_tables(
        &self,
        connection_id: &str,
        schema: &str,
    ) -> AppResult<Vec<TableInfo>> {
        let (profile, _guard, cancelled) = self
            .start_operation(connection_id, &Uuid::new_v4().to_string())
            .await?;
        let profile = profile.postgres()?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                let (_, result) = tokio::try_join!(db.preflight(), db.tables(schema))?;
                profile.recycle(db);
                Ok(result)
            },
            cancelled,
        )
        .await
    }

    pub async fn redis_command(&self, request: RedisRequest) -> AppResult<RedisResult> {
        let (profile, _guard, cancelled) = self
            .start_operation(&request.connection_id, &request.request_id)
            .await?;
        let Adapter::Redis(p) = &profile.adapter else {
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
        if let Adapter::Mysql(p) = &profile.adapter {
            return bounded_metadata(mysql::overview(p), cancelled).await;
        }
        let profile = profile.postgres()?;
        bounded_metadata(
            async {
                let db = profile.open().await?;
                let (_, result) = tokio::try_join!(db.preflight(), db.overview())?;
                profile.recycle(db);
                Ok(result)
            },
            cancelled,
        )
        .await
    }
}

async fn bounded_metadata<T>(
    work: impl Future<Output = AppResult<T>>,
    cancelled: watch::Receiver<bool>,
) -> AppResult<T> {
    bounded_metadata_until(
        work,
        cancelled,
        tokio::time::Instant::now() + Duration::from_secs(15),
    )
    .await
}

async fn bounded_metadata_until<T>(
    work: impl Future<Output = AppResult<T>>,
    mut cancelled: watch::Receiver<bool>,
    deadline: tokio::time::Instant,
) -> AppResult<T> {
    tokio::select! {
        result = work => result,
        _ = async { let _ = cancelled.wait_for(|value| *value).await; } => Err(AppError::new("CANCELLED", "Metadata connection closed after disconnect.")),
        _ = tokio::time::sleep_until(deadline) => Err(AppError::new("TIMEOUT", "Metadata operation timed out and its connection was closed.")),
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
