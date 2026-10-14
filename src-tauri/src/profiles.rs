//! Device-local connection bookmarks. This format never contains passwords or query data.
use crate::models::*;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::Mutex};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedTab {
    pub schema: String,
    pub table: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileLayout {
    pub selected_schema: String,
    pub tabs: Vec<SavedTab>,
    pub active_tab: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedConnection {
    pub id: String,
    pub engine: Engine,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub schema: String,
    pub tls: TlsMode,
    pub root_ca_pem: String,
    pub manual_edits: bool,
    pub group: String,
    pub pinned: bool,
    pub last_used: u64,
    pub has_password: bool,
    pub layout: ProfileLayout,
}

impl SavedConnection {
    pub fn from_request(request: &ConnectRequest, id: Option<String>, group: String) -> Self {
        Self {
            id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            engine: request.engine,
            name: request.name.clone(),
            host: request.host.clone(),
            port: request.port,
            database: request.database.clone(),
            username: request.username.clone(),
            schema: request.schema.clone(),
            tls: request.tls,
            root_ca_pem: request.root_ca_pem.clone(),
            manual_edits: request.manual_edits,
            group,
            pinned: false,
            last_used: now(),
            has_password: false,
            layout: ProfileLayout {
                selected_schema: request.schema.clone(),
                ..Default::default()
            },
        }
    }

    pub fn request(&self, password: String) -> ConnectRequest {
        ConnectRequest {
            engine: self.engine,
            name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            database: self.database.clone(),
            username: self.username.clone(),
            password,
            schema: self.schema.clone(),
            tls: self.tls,
            root_ca_pem: self.root_ca_pem.clone(),
            manual_edits: self.manual_edits,
        }
    }

    fn validate(&self) -> AppResult<()> {
        if Uuid::parse_str(&self.id).is_err()
            || self.port == 0
            || self.group.len() > 100
            || [&self.name, &self.host, &self.database]
                .iter()
                .any(|s| s.is_empty())
            || [
                &self.name,
                &self.host,
                &self.database,
                &self.username,
                &self.schema,
            ]
            .iter()
            .any(|s| s.len() > 253 || s.contains('\0'))
            || self.root_ca_pem.len() > 65536
            || self.root_ca_pem.contains("PRIVATE KEY")
        {
            return Err(AppError::new(
                "INVALID_PROFILE",
                "Invalid saved connection settings.",
            ));
        }
        validate_layout(&self.layout)
    }
}

fn validate_layout(layout: &ProfileLayout) -> AppResult<()> {
    if layout.tabs.len() > 40
        || layout.selected_schema.len() > 253
        || layout.active_tab.len() > 1024
        || layout.tabs.iter().any(|t| {
            t.schema.len() > 253
                || t.table.len() > 253
                || t.schema.contains('\0')
                || t.table.contains('\0')
        })
    {
        return Err(AppError::new(
            "INVALID_LAYOUT",
            "At most 40 table tabs can be remembered per connection.",
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileData {
    version: u32,
    connections: Vec<SavedConnection>,
}

pub struct Profiles {
    path: PathBuf,
    lock: Mutex<()>,
}
impl Profiles {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            path: directory.join("connections.json"),
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> AppResult<Vec<SavedConnection>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        if fs::metadata(&self.path).map_err(|_| storage_error())?.len() > 4 * 1024 * 1024 {
            return Err(storage_error());
        }
        let bytes = fs::read(&self.path).map_err(|_| storage_error())?;
        let data: FileData = serde_json::from_slice(&bytes).map_err(|_| AppError::new("PROFILE_FILE_INVALID", "Saved connection file is unreadable or uses a newer format. It has not been overwritten."))?;
        if data.version != 1 || data.connections.len() > 500 {
            return Err(storage_error());
        }
        let mut ids = std::collections::HashSet::new();
        for profile in &data.connections {
            profile.validate()?;
            if !ids.insert(&profile.id) {
                return Err(storage_error());
            }
        }
        Ok(data.connections)
    }

    fn write(&self, connections: Vec<SavedConnection>) -> AppResult<()> {
        if connections.len() > 500 {
            return Err(AppError::new(
                "PROFILE_LIMIT",
                "Maximum 500 saved connections. Remove unused bookmarks first.",
            ));
        }
        let parent = self.path.parent().ok_or_else(storage_error)?;
        fs::create_dir_all(parent).map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .map_err(|_| storage_error())?;
        }
        let bytes = serde_json::to_vec_pretty(&FileData {
            version: 1,
            connections,
        })
        .map_err(|_| storage_error())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(storage_error());
        }
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| storage_error())?;
        file.write_all(&bytes).map_err(|_| storage_error())?;
        file.as_file().sync_all().map_err(|_| storage_error())?;
        file.persist(&self.path).map_err(|_| storage_error())?;
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<SavedConnection>> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        self.read()
    }
    pub fn get(&self, id: &str) -> AppResult<SavedConnection> {
        self.list()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::new("PROFILE_NOT_FOUND", "Saved connection no longer exists."))
    }
    pub fn put(&self, mut profile: SavedConnection) -> AppResult<SavedConnection> {
        profile.validate()?;
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut profiles = self.read()?;
        if let Some(old) = profiles.iter_mut().find(|p| p.id == profile.id) {
            profile.pinned = old.pinned;
            profile.layout = old.layout.clone();
            *old = profile.clone();
        } else {
            profiles.push(profile.clone());
        }
        self.write(profiles)?;
        Ok(profile)
    }
    pub fn update(
        &self,
        id: &str,
        layout: ProfileLayout,
        pinned: bool,
        group: String,
    ) -> AppResult<()> {
        validate_layout(&layout)?;
        if group.len() > 100 {
            return Err(AppError::new("INVALID_PROFILE", "Group name is too long."));
        }
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut profiles = self.read()?;
        let profile = profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(storage_error)?;
        profile.last_used = now();
        profile.layout = layout;
        profile.pinned = pinned;
        profile.group = group;
        self.write(profiles)
    }
    pub fn remove(&self, id: &str) -> AppResult<()> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut profiles = self.read()?;
        profiles.retain(|p| p.id != id);
        self.write(profiles)
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn storage_error() -> AppError {
    AppError::new(
        "PROFILE_STORAGE",
        "Could not read or save local connections. Check app data folder access and free space.",
    )
}

#[cfg(feature = "desktop")]
pub fn credential(id: &str) -> AppResult<keyring_core::Entry> {
    if Uuid::parse_str(id).is_err() {
        return Err(AppError::new("INVALID_PROFILE", "Invalid connection ID."));
    }
    #[cfg(target_os = "macos")]
    {
        use keyring_core::api::CredentialStoreApi;
        // File-based login Keychain, not the iCloud/protected-data store.
        let store =
            apple_native_keyring_store::keychain::Store::new().map_err(|_| vault_error())?;
        store
            .build("dev.tablepor.desktop.connections", id, None)
            .map_err(|_| vault_error())
    }
    #[cfg(target_os = "windows")]
    {
        use keyring_core::api::CredentialStoreApi;
        let store = windows_native_keyring_store::Store::new().map_err(|_| vault_error())?;
        let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
        store
            .build("dev.tablepor.desktop.connections", id, Some(&modifiers))
            .map_err(|_| vault_error())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err(vault_error())
}
#[cfg(feature = "desktop")]
pub fn vault_error() -> AppError {
    AppError::new(
        "CREDENTIAL_STORE",
        "The system password store is unavailable. Enter the password manually or unlock the system password store.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> SavedConnection {
        SavedConnection::from_request(
            &ConnectRequest {
                engine: Engine::Postgres,
                name: "Fixture".into(),
                host: "127.0.0.1".into(),
                port: 5432,
                database: "fixture".into(),
                username: "reader".into(),
                password: "CANARY_DO_NOT_PERSIST".into(),
                schema: "public".into(),
                tls: TlsMode::LocalOnly,
                root_ca_pem: String::new(),
                manual_edits: false,
            },
            None,
            "Tests".into(),
        )
    }
    #[test]
    fn bookmarks_survive_restart_without_passwords_or_rows() {
        let dir = tempfile::tempdir().unwrap();
        let store = Profiles::new(dir.path().to_owned());
        let p = store.put(fixture()).unwrap();
        let layout = ProfileLayout {
            selected_schema: "audit".into(),
            tabs: vec![SavedTab {
                schema: "audit".into(),
                table: "events".into(),
            }],
            active_tab: "audit.events".into(),
        };
        store
            .update(&p.id, layout, true, "Production".into())
            .unwrap();
        let restarted = Profiles::new(dir.path().to_owned());
        let saved = restarted.get(&p.id).unwrap();
        assert!(saved.pinned);
        assert_eq!(saved.layout.selected_schema, "audit");
        let content = fs::read_to_string(dir.path().join("connections.json")).unwrap();
        assert!(!content.contains("CANARY_DO_NOT_PERSIST"));
        assert!(!content.contains("\"password\""));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join("connections.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        restarted.remove(&p.id).unwrap();
        assert!(restarted.list().unwrap().is_empty());
    }
    #[test]
    fn invalid_files_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = Profiles::new(dir.path().to_owned());
        fs::write(&store.path, b"not json").unwrap();
        assert!(store.put(fixture()).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), b"not json");
    }
}

#[cfg(all(
    test,
    feature = "desktop",
    any(target_os = "macos", target_os = "windows")
))]
mod native_vault_tests {
    use super::*;
    #[test]
    #[ignore = "creates and deletes one disposable OS credential"]
    fn local_credential_roundtrip() {
        let id = Uuid::new_v4().to_string();
        let entry = credential(&id).unwrap();
        entry.set_password("disposable-buggy-table-test").unwrap();
        let value = entry.get_password().unwrap();
        #[cfg(target_os = "windows")]
        let persistence = entry.get_attributes().unwrap().get("persistence").cloned();
        entry.delete_credential().unwrap();
        assert_eq!(value, "disposable-buggy-table-test");
        assert!(entry.get_password().is_err());
        #[cfg(target_os = "windows")]
        assert_eq!(persistence.as_deref(), Some("Local"));
    }
}
