use crate::{
    models::*,
    profiles::{Profiles, SavedConnection},
    service::Workbench,
    vpn::{
        runtime::{RuntimeInfo, VpnRuntime, VpnStatus},
        store::VpnStore,
        *,
    },
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

// Serialize profile changes with tunnel startup so edits cannot race a launch.
pub(crate) static CHANGES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn storage<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&VpnStore) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<VpnStore>()))
        .await
        .map_err(|_| failure("VPN_STORAGE", "VPN settings could not be read."))?
}

pub(crate) async fn connect_network(
    app: &AppHandle,
    state: &Workbench,
    request: ConnectRequest,
    owner: &str,
    selection: VpnSelection,
    replacing: Option<&str>,
) -> AppResult<ConnectionInfo> {
    selection.validate()?;
    if !selection.enabled {
        return state
            .connect_route(request, None, replacing, Some(owner.to_string()))
            .await;
    }
    let _lock = CHANGES.lock().await;
    let profile_id = selection
        .profile_id
        .clone()
        .ok_or_else(|| failure("VPN_PROFILE", "Choose a default VPN."))?;
    let saved_owner = owner.to_string();
    let (profile, secrets) = storage(app.clone(), move |store| {
        store.load(&saved_owner, &profile_id)
    })
    .await?;
    let route = app
        .state::<VpnRuntime>()
        .start(profile, secrets, &request.host, request.port)
        .await?;
    state
        .connect_route(request, Some(route), replacing, Some(owner.to_string()))
        .await
}

#[tauri::command]
pub async fn vpn_profiles(app: AppHandle, owner: String) -> AppResult<Vec<VpnProfile>> {
    storage(app, move |store| store.list(&owner)).await
}
#[tauri::command]
pub async fn discard_vpn_draft(
    app: AppHandle,
    state: State<'_, Workbench>,
    owner: String,
) -> AppResult<()> {
    let _lock = CHANGES.lock().await;
    if state.vpn_owner_in_use(&owner).await {
        return Ok(());
    }
    let saved = app.state::<Profiles>().get(&owner);
    match saved {
        Ok(_) => Ok(()),
        Err(e) if e.code == "PROFILE_NOT_FOUND" => {
            storage(app, move |store| store.remove_owner(&owner)).await
        }
        Err(e) => Err(e),
    }
}
#[tauri::command]
pub async fn save_vpn_profile(
    app: AppHandle,
    state: State<'_, Workbench>,
    edit: VpnEdit,
) -> AppResult<VpnProfile> {
    let _lock = CHANGES.lock().await;
    if let Some(id) = &edit.id
        && state.vpn_profile_in_use(id).await
    {
        return Err(failure(
            "VPN_IN_USE",
            "Disconnect this database before editing its profile.",
        ));
    }
    storage(app, move |store| store.put(edit)).await
}
#[tauri::command]
pub async fn remove_vpn_profile(
    app: AppHandle,
    state: State<'_, Workbench>,
    owner: String,
    id: String,
) -> AppResult<Option<SavedConnection>> {
    let _lock = CHANGES.lock().await;
    if state.vpn_profile_in_use(&id).await {
        return Err(failure(
            "VPN_IN_USE",
            "Disconnect this database before removing its profile.",
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let saved = match app.state::<Profiles>().get(&owner) {
            Ok(p) if p.vpn.profile_id.as_deref() == Some(&id) => Some(
                app.state::<Profiles>()
                    .set_vpn(&owner, VpnSelection::default())?,
            ),
            Ok(p) => Some(p),
            Err(e) if e.code == "PROFILE_NOT_FOUND" => None,
            Err(e) => return Err(e),
        };
        app.state::<VpnStore>().remove(&owner, &id)?;
        Ok(saved)
    })
    .await
    .map_err(|_| failure("VPN_STORAGE", "Could not remove VPN profile."))?
}
#[tauri::command]
pub async fn vpn_runtime(state: State<'_, VpnRuntime>) -> AppResult<RuntimeInfo> {
    Ok(state.info().await)
}
#[tauri::command]
pub async fn prepare_vpn(state: State<'_, VpnRuntime>) -> AppResult<()> {
    state.prepare().await
}
#[tauri::command]
pub async fn vpn_status(
    state: State<'_, Workbench>,
    connection_id: String,
    probe: bool,
) -> AppResult<VpnStatus> {
    state.vpn_status(&connection_id, probe).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedNetwork {
    connection: Option<ConnectionInfo>,
    profile: Option<SavedConnection>,
    warning: Option<String>,
}
#[tauri::command]
pub async fn set_connection_vpn(
    app: AppHandle,
    state: State<'_, Workbench>,
    owner: String,
    selection: VpnSelection,
    connection_id: Option<String>,
) -> AppResult<ChangedNetwork> {
    selection.validate()?;
    if uuid::Uuid::parse_str(&owner).is_err() {
        return Err(failure("VPN_PROFILE", "Invalid connection ID."));
    }
    if let Some(id) = selection.profile_id.clone() {
        let owner = owner.clone();
        // Verify ownership/existence without returning any secret to the frontend.
        storage(app.clone(), move |store| {
            if store.list(&owner)?.iter().any(|p| p.id == id) {
                Ok(())
            } else {
                Err(failure(
                    "VPN_PROFILE",
                    "Choose an existing VPN profile for this connection.",
                ))
            }
        })
        .await?;
    }
    let connection = if let Some(id) = connection_id {
        state.check_owner(&id, &owner).await?;
        let request = state.connection_request(&id).await?;
        Some(connect_network(&app, &state, request, &owner, selection.clone(), Some(&id)).await?)
    } else {
        None
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Profiles>();
        match store.get(&owner) {
            Ok(_) => store.set_vpn(&owner, selection).map(Some),
            Err(e) if e.code == "PROFILE_NOT_FOUND" => Ok(None),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|_| failure("VPN_STORAGE", "VPN preferences could not be saved."))?;
    match result {
        Ok(profile) => Ok(ChangedNetwork {
            connection,
            profile,
            warning: None,
        }),
        Err(e) if connection.is_some() => Ok(ChangedNetwork {
            connection,
            profile: None,
            warning: Some(format!(
                "Network changed for this session, but the preference was not saved. {}",
                e.message
            )),
        }),
        Err(e) => Err(e),
    }
}
