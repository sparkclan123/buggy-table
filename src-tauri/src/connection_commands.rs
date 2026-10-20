use crate::{
    models::*,
    profiles::{self, ProfileLayout, Profiles, SavedConnection},
    service::Workbench,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedProfile {
    pub(crate) connection: ConnectionInfo,
    pub(crate) profile: Option<SavedConnection>,
    pub(crate) warning: Option<String>,
}

async fn storage<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&Profiles) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<Profiles>()))
        .await
        .map_err(|_| {
            AppError::new(
                "PROFILE_STORAGE",
                "Local connection storage operation failed.",
            )
        })?
}

#[tauri::command]
pub async fn list_connections(app: AppHandle) -> AppResult<Vec<SavedConnection>> {
    storage(app, |store| store.list()).await
}

#[tauri::command]
pub async fn connect_profile(
    app: AppHandle,
    request: ConnectRequest,
    saved_id: Option<String>,
    group: String,
    remember: bool,
    save_password: bool,
    vpn: Option<crate::vpn::VpnSelection>,
) -> AppResult<ConnectedProfile> {
    let state = app.state::<Workbench>();
    let id = saved_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(AppError::new("INVALID_PROFILE", "Invalid connection ID."));
    }
    let old_id = id.clone();
    let old = storage(app.clone(), move |store| match store.get(&old_id) {
        Ok(p) => Ok(Some(p)),
        Err(e) if e.code == "PROFILE_NOT_FOUND" => Ok(None),
        Err(e) => Err(e),
    })
    .await?;
    let selection = vpn.unwrap_or_else(|| old.map(|p| p.vpn).unwrap_or_default());
    selection.validate()?;
    let secret = Zeroizing::new(request.password.clone());
    let mut profile = SavedConnection::from_request(&request, Some(id.clone()), group);
    profile.vpn = selection.clone();
    let connection =
        crate::vpn_commands::connect_network(&app, &state, request, &id, selection, None).await?;
    if !remember {
        return Ok(ConnectedProfile {
            connection,
            profile: None,
            warning: None,
        });
    }
    let result = storage(app, move |store| {
        let _lock = VAULT_LOCK.lock().map_err(|_| profiles::vault_error())?;
        // Commit the password-free bookmark first. A locked keychain must not lose it.
        profile.has_password = false;
        let mut saved = store.put(profile)?;
        let secret_result = if save_password {
            profiles::credential(&saved.id).and_then(|entry| {
                entry
                    .set_password(&secret)
                    .map_err(|_| profiles::vault_error())
            })
        } else if cfg!(any(target_os = "macos", target_os = "windows")) {
            forget_secret(&saved.id)
        } else {
            Ok(())
        };
        if let Err(error) = secret_result {
            return Ok((saved, Some(error.message)));
        }
        if save_password {
            saved.has_password = true;
            if let Err(error) = store.put(saved.clone()) {
                let _ = forget_secret(&saved.id);
                saved.has_password = false;
                return Ok((saved, Some(error.message)));
            }
        }
        Ok((saved, None))
    })
    .await;
    match result {
        Ok((profile, warning)) => Ok(ConnectedProfile {
            connection,
            profile: Some(profile),
            warning,
        }),
        Err(error) => Ok(ConnectedProfile {
            connection,
            profile: None,
            warning: Some(format!(
                "Connected for this session, but settings could not be saved. {}",
                error.message
            )),
        }),
    }
}

static VAULT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn forget_secret(id: &str) -> AppResult<()> {
    match profiles::credential(id)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(_) => Err(profiles::vault_error()),
    }
}

#[tauri::command]
pub async fn reconnect_profile(
    app: AppHandle,
    state: State<'_, Workbench>,
    id: String,
) -> AppResult<ConnectedProfile> {
    let (profile, password) = storage(app.clone(), move |store| {
        let profile = store.get(&id)?;
        if !profile.has_password {
            return Err(AppError::new(
                "PASSWORD_REQUIRED",
                "Enter the password to reconnect.",
            ));
        }
        let _lock = VAULT_LOCK.lock().map_err(|_| profiles::vault_error())?;
        let secret = profiles::credential(&id)?
            .get_password()
            .map_err(|_| profiles::vault_error())?;
        Ok((profile, Zeroizing::new(secret)))
    })
    .await?;
    // The password is sent only to the saved endpoint through the database adapter.
    let connection = crate::vpn_commands::connect_network(
        &app,
        &state,
        profile.request(password.to_string()),
        &profile.id,
        profile.vpn.clone(),
        None,
    )
    .await?;
    let mut profile = profile;
    profile.last_used = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let result = storage(app, move |store| store.put(profile)).await;
    match result {
        Ok(profile) => Ok(ConnectedProfile {
            connection,
            profile: Some(profile),
            warning: None,
        }),
        Err(error) => Ok(ConnectedProfile {
            connection,
            profile: None,
            warning: Some(error.message),
        }),
    }
}

#[tauri::command]
pub async fn update_connection(
    app: AppHandle,
    id: String,
    layout: ProfileLayout,
    pinned: bool,
    group: String,
) -> AppResult<()> {
    storage(app, move |store| store.update(&id, layout, pinned, group)).await
}

#[tauri::command]
pub async fn remove_connection(app: AppHandle, id: String) -> AppResult<()> {
    let _changes = crate::vpn_commands::CHANGES.write().await;
    if app.state::<Workbench>().vpn_owner_in_use(&id).await {
        return Err(AppError::new(
            "CONNECTION_IN_USE",
            "Disconnect this database before removing it.",
        ));
    }
    let vpn_app = app.clone();
    storage(app, move |store| {
        let profile = store.get(&id)?;
        let _lock = VAULT_LOCK.lock().map_err(|_| profiles::vault_error())?;
        if profile.has_password || cfg!(any(target_os = "macos", target_os = "windows")) {
            forget_secret(&id)?;
        }
        vpn_app
            .state::<crate::vpn::store::VpnStore>()
            .remove_owner(&id)?;
        store.remove(&id)
    })
    .await
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, Workbench>,
    connection_id: String,
) -> AppResult<Vec<String>> {
    state.schemas(&connection_id).await
}

#[tauri::command]
pub async fn schema_tables(
    state: State<'_, Workbench>,
    connection_id: String,
    schema: String,
) -> AppResult<Vec<TableInfo>> {
    state.schema_tables(&connection_id, &schema).await
}
