//! The webview can check/install, but cannot choose a server, key or artifact URL.
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

use crate::{
    models::{AppError, AppResult},
    service::Workbench,
};

const RELEASES: &str = "https://github.com/sparkclan123/buggy-table-releases/releases/download/";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;

#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    configured: bool,
    current_version: String,
    version: Option<String>,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    stage: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

fn configured(app: &AppHandle) -> bool {
    !cfg!(debug_assertions)
        && app
            .config()
            .plugins
            .0
            .get("updater")
            .and_then(|config| config.get("pubkey"))
            .and_then(|key| key.as_str())
            .is_some_and(|key| !key.trim().is_empty())
}

#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    state: State<'_, Updates>,
    fetch: bool,
) -> AppResult<UpdateStatus> {
    let mut status = UpdateStatus {
        configured: configured(&app),
        current_version: app.package_info().version.to_string(),
        version: None,
        notes: None,
    };
    if !status.configured || !fetch {
        return Ok(status);
    }
    let mut pending = state
        .pending
        .try_lock()
        .map_err(|_| AppError::new("UPDATE_BUSY", "An update operation is already running."))?;
    // Invalidate an old offer before a fresh check, including on network failure.
    *pending = None;
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| AppError::new("UPDATE_CONFIG", "Update configuration is unavailable."))?;
    let update = updater.check().await.map_err(|_| {
        AppError::new(
            "UPDATE_CHECK",
            "Could not check GitHub for updates. Try again later.",
        )
    })?;
    if let Some(mut update) = update {
        if !update.download_url.as_str().starts_with(RELEASES) {
            return Err(AppError::new(
                "UPDATE_SOURCE",
                "The update download is outside the approved release repository.",
            ));
        }
        update.timeout = Some(Duration::from_secs(180));
        status.version = Some(update.version.clone());
        status.notes = update
            .body
            .as_ref()
            .map(|notes| notes.chars().take(4000).collect());
        *pending = Some(update);
    }
    Ok(status)
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, Updates>,
    workbench: State<'_, Workbench>,
    progress: Channel<Progress>,
) -> AppResult<()> {
    let pending = state
        .pending
        .try_lock()
        .map_err(|_| AppError::new("UPDATE_BUSY", "An update operation is already running."))?;
    let update = pending
        .as_ref()
        .ok_or_else(|| AppError::new("NO_UPDATE", "Check for updates before installing."))?;
    let _maintenance = workbench.begin_update()?;
    let (oversized, mut too_large) = tokio::sync::watch::channel(false);
    let mut downloaded = 0u64;
    let mut last_progress = Instant::now() - Duration::from_secs(1);
    let download = update.download(
        |chunk, total| {
            downloaded = downloaded.saturating_add(chunk as u64);
            if downloaded > MAX_DOWNLOAD || total.is_some_and(|n| n > MAX_DOWNLOAD) {
                let _ = oversized.send(true);
            }
            if last_progress.elapsed() >= Duration::from_millis(100) {
                let _ = progress.send(Progress {
                    stage: "downloading",
                    downloaded,
                    total,
                });
                last_progress = Instant::now();
            }
        },
        || {},
    );
    let bytes = tokio::select! {
        biased;
        _ = too_large.changed() => return Err(AppError::new("UPDATE_SIZE", "The update exceeds the download size limit.")),
        result = download => result.map_err(|_| AppError::new("UPDATE_DOWNLOAD", "Download or signature verification failed. Nothing was installed; try again."))?,
    };
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err(AppError::new(
            "UPDATE_SIZE",
            "The update exceeds the download size limit.",
        ));
    }
    // download() verifies the mandatory signature before any installation starts.
    let _ = progress.send(Progress {
        stage: "installing",
        downloaded: bytes.len() as u64,
        total: Some(bytes.len() as u64),
    });
    update.install(&bytes).map_err(|_| {
        AppError::new(
            "UPDATE_INSTALL",
            "Installation failed. Close other copies of buggy-table and try again.",
        )
    })?;
    app.restart();
}
