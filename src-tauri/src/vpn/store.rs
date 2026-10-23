use super::*;
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, AeadCore, OsRng, Payload},
};
use std::{fs, io::Write, path::PathBuf, sync::Mutex};
use zeroize::Zeroizing;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    profile: VpnProfile,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileData {
    version: u8,
    profiles: Vec<Record>,
}

pub struct VpnStore {
    path: PathBuf,
    lock: Mutex<()>,
    #[cfg(test)]
    key: Option<[u8; 32]>,
}
impl VpnStore {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            path: directory.join("vpn-profiles.json"),
            lock: Mutex::new(()),
            #[cfg(test)]
            key: None,
        }
    }
    fn key(&self, create: bool) -> AppResult<Zeroizing<Vec<u8>>> {
        #[cfg(test)]
        if let Some(key) = self.key {
            return Ok(Zeroizing::new(key.to_vec()));
        }
        #[cfg(feature = "desktop")]
        {
            let entry = crate::profiles::credential_for(
                "dev.tablepor.desktop.vpn",
                "f22f79c2-553d-4b92-8bee-596cbf760d41",
            )?;
            let encoded = match entry.get_password() {
                Ok(value) => Zeroizing::new(value),
                Err(keyring_core::Error::NoEntry) if create => {
                    let key = Zeroizing::new(format!(
                        "{}{}",
                        uuid::Uuid::new_v4().simple(),
                        uuid::Uuid::new_v4().simple()
                    ));
                    entry
                        .set_password(&key)
                        .map_err(|_| crate::profiles::vault_error())?;
                    key
                }
                Err(_) => return Err(crate::profiles::vault_error()),
            };
            if encoded.len() != 64 || !encoded.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(storage_error());
            }
            let key = (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).map_err(|_| storage_error()))
                .collect::<AppResult<Vec<_>>>()?;
            Ok(Zeroizing::new(key))
        }
        #[cfg(not(feature = "desktop"))]
        {
            let _ = create;
            Err(failure(
                "VPN_VAULT",
                "VPN profiles require the desktop system credential store.",
            ))
        }
    }
    fn read(&self) -> AppResult<FileData> {
        if !self.path.exists() {
            return Ok(FileData {
                version: 1,
                profiles: vec![],
            });
        }
        if fs::metadata(&self.path).map_err(|_| storage_error())?.len() > 32 * 1024 * 1024 {
            return Err(storage_error());
        }
        let data: FileData =
            serde_json::from_slice(&fs::read(&self.path).map_err(|_| storage_error())?)
                .map_err(|_| storage_error())?;
        if data.version != 1 || data.profiles.len() > 50 {
            return Err(storage_error());
        }
        let mut ids = std::collections::HashSet::new();
        for r in &data.profiles {
            validate_meta(
                &r.profile.owner,
                &r.profile.name,
                &r.profile.username,
                &r.profile.dns,
            )?;
            if uuid::Uuid::parse_str(&r.profile.id).is_err()
                || !ids.insert(&r.profile.id)
                || r.nonce.len() != 12
                || r.ciphertext.len() > 600 * 1024
            {
                return Err(storage_error());
            }
        }
        Ok(data)
    }
    fn write(&self, data: &FileData) -> AppResult<()> {
        let parent = self.path.parent().ok_or_else(storage_error)?;
        fs::create_dir_all(parent).map_err(|_| storage_error())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .map_err(|_| storage_error())?;
        }
        let bytes = serde_json::to_vec(data).map_err(|_| storage_error())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(storage_error());
        }
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| storage_error())?;
        file.write_all(&bytes).map_err(|_| storage_error())?;
        file.as_file().sync_all().map_err(|_| storage_error())?;
        file.persist(&self.path).map_err(|_| storage_error())?;
        Ok(())
    }
    pub fn list(&self, owner: &str) -> AppResult<Vec<VpnProfile>> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        Ok(self
            .read()?
            .profiles
            .into_iter()
            .filter(|r| r.profile.owner == owner)
            .map(|r| r.profile)
            .collect())
    }
    fn decrypt(&self, record: &Record, key: &[u8]) -> AppResult<VpnSecrets> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| storage_error())?;
        let aad = format!("{}:{}", record.profile.owner, record.profile.id);
        let bytes = Zeroizing::new(
            cipher
                .decrypt(
                    record.nonce.as_slice().into(),
                    Payload {
                        msg: &record.ciphertext,
                        aad: aad.as_bytes(),
                    },
                )
                .map_err(|_| storage_error())?,
        );
        serde_json::from_slice(&bytes).map_err(|_| storage_error())
    }
    pub fn load(&self, owner: &str, id: &str) -> AppResult<(VpnProfile, VpnSecrets)> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let data = self.read()?;
        let record = data
            .profiles
            .iter()
            .find(|r| r.profile.id == id && r.profile.owner == owner)
            .ok_or_else(|| {
                failure(
                    "VPN_PROFILE",
                    "The selected VPN profile no longer exists. Open VPN settings.",
                )
            })?;
        Ok((
            record.profile.clone(),
            self.decrypt(record, &self.key(false)?)?,
        ))
    }
    pub fn put(&self, mut edit: VpnEdit) -> AppResult<VpnProfile> {
        validate_meta(&edit.owner, &edit.name, &edit.username, &edit.dns)?;
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut data = self.read()?;
        let key = self.key(data.profiles.is_empty())?;
        let existing = edit.id.as_ref().and_then(|id| {
            data.profiles
                .iter()
                .position(|r| &r.profile.id == id && r.profile.owner == edit.owner)
        });
        if edit.id.is_some() && existing.is_none() {
            return Err(failure("VPN_PROFILE", "VPN profile no longer exists."));
        }
        if existing.is_none() && data.profiles.len() >= 50 {
            return Err(failure(
                "VPN_LIMIT",
                "Remove an unused VPN profile first (maximum 50).",
            ));
        }
        let mut secrets = match existing {
            Some(i) => self.decrypt(&data.profiles[i], &key)?,
            None => VpnSecrets {
                config: String::new(),
                password: String::new(),
                key_password: String::new(),
            },
        };
        if let Some(config) = edit.config.as_ref() {
            secrets.config = validate_config(config)?;
        }
        if secrets.config.is_empty() {
            return Err(failure(
                "VPN_CONFIG",
                "Choose an inline .ovpn client profile.",
            ));
        }
        if let Some(password) = edit.password.take() {
            secrets.password = password;
        }
        if let Some(password) = edit.key_password.take() {
            secrets.key_password = password;
        }
        for value in [&secrets.password, &secrets.key_password] {
            if value.len() > 4096 || value.contains(['\n', '\r', '\0']) {
                return Err(failure(
                    "VPN_PROFILE",
                    "VPN passwords must be one line and at most 4096 bytes.",
                ));
            }
        }
        let requires_auth = secrets
            .config
            .lines()
            .any(|s| s.trim() == "auth-user-pass" || s.trim() == "--auth-user-pass");
        if requires_auth && edit.username.is_empty() {
            return Err(failure(
                "VPN_AUTH",
                "This OpenVPN profile requires a VPN username and password.",
            ));
        }
        let profile = VpnProfile {
            id: edit
                .id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            owner: edit.owner.clone(),
            name: edit.name.trim().into(),
            username: edit.username.clone(),
            dns: edit.dns.clone(),
            has_password: !secrets.password.is_empty(),
            has_key_password: !secrets.key_password.is_empty(),
        };
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| storage_error())?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let bytes = Zeroizing::new(serde_json::to_vec(&secrets).map_err(|_| storage_error())?);
        let aad = format!("{}:{}", profile.owner, profile.id);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &bytes,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| storage_error())?;
        let record = Record {
            profile: profile.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
        };
        if let Some(i) = existing {
            data.profiles[i] = record;
        } else {
            data.profiles.push(record);
        }
        self.write(&data)?;
        Ok(profile)
    }
    pub fn remove(&self, owner: &str, id: &str) -> AppResult<()> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut data = self.read()?;
        data.profiles
            .retain(|r| r.profile.owner != owner || r.profile.id != id);
        self.write(&data)
    }
    pub fn remove_owner(&self, owner: &str) -> AppResult<()> {
        let _lock = self.lock.lock().map_err(|_| storage_error())?;
        let mut data = self.read()?;
        data.profiles.retain(|r| r.profile.owner != owner);
        self.write(&data)
    }
}
fn storage_error() -> AppError {
    failure(
        "VPN_STORAGE",
        "Could not read or save encrypted VPN settings. Unlock the system credential store and check app data access. Existing files were not replaced with defaults.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONFIG: &str = "client\ndev tun\nremote example.invalid 1194 udp\n<ca>\nTEST_CERTIFICATE\n</ca>\n<key>\nPRIVATE_CANARY\n</key>\n";
    #[test]
    fn encrypted_profiles_are_owned_and_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VpnStore::new(dir.path().into());
        store.key = Some([7; 32]);
        let owner = uuid::Uuid::new_v4().to_string();
        let p = store
            .put(VpnEdit {
                owner: owner.clone(),
                id: None,
                name: "Office".into(),
                config: Some(format!(
                    "{CONFIG}push-peer-info\nignore-unknown-option block-outside-dns\n"
                )),
                username: "user".into(),
                password: Some("PASSWORD_CANARY".into()),
                key_password: None,
                dns: vec![],
            })
            .unwrap();
        let bytes = fs::read_to_string(&store.path).unwrap();
        assert!(!bytes.contains("PRIVATE_CANARY") && !bytes.contains("PASSWORD_CANARY"));
        assert!(
            store
                .load(&uuid::Uuid::new_v4().to_string(), &p.id)
                .is_err()
        );
        let mut restarted = VpnStore::new(dir.path().into());
        restarted.key = Some([7; 32]);
        assert_eq!(
            restarted.load(&owner, &p.id).unwrap().1.password,
            "PASSWORD_CANARY"
        );
        assert!(
            restarted
                .load(&owner, &p.id)
                .unwrap()
                .1
                .config
                .lines()
                .any(|line| line == "push-peer-info")
        );
        assert!(
            restarted
                .load(&owner, &p.id)
                .unwrap()
                .1
                .config
                .lines()
                .any(|line| line == "ignore-unknown-option block-outside-dns")
        );
        restarted
            .put(VpnEdit {
                owner: owner.clone(),
                id: Some(p.id.clone()),
                name: "Renamed".into(),
                config: None,
                username: "user".into(),
                password: None,
                key_password: None,
                dns: vec![],
            })
            .unwrap();
        assert_eq!(
            restarted.load(&owner, &p.id).unwrap().1.password,
            "PASSWORD_CANARY"
        );
        let mut wrong = VpnStore::new(dir.path().into());
        wrong.key = Some([8; 32]);
        assert!(wrong.load(&owner, &p.id).is_err());
        let before = fs::read(&store.path).unwrap();
        assert!(
            wrong
                .put(VpnEdit {
                    owner: owner.clone(),
                    id: Some(p.id.clone()),
                    name: "No".into(),
                    config: None,
                    username: "".into(),
                    password: None,
                    key_password: None,
                    dns: vec![]
                })
                .is_err()
        );
        assert_eq!(before, fs::read(&store.path).unwrap());
        restarted.remove(&owner, &p.id).unwrap();
        assert!(restarted.list(&owner).unwrap().is_empty());
    }
    #[test]
    fn peer_info_profiles_are_imported_without_dropping_the_option() {
        for directive in [
            "push-peer-info",
            "--push-peer-info",
            "  push-peer-info  ",
            "push-peer-info # exported client metadata",
        ] {
            // Include Windows line endings and a BOM as found in exported profiles.
            let input = format!("\u{feff}{CONFIG}{directive}\n").replace('\n', "\r\n");
            let validated = validate_config(&input).unwrap();
            assert!(
                validated.lines().any(|line| line == directive.trim()),
                "{directive}"
            );
            assert!(validated.contains("remote-cert-tls server\n"));
        }
    }
    #[test]
    fn peer_info_does_not_accept_arguments_or_bypass_profile_restrictions() {
        for option in [
            "push-peer-info unexpected",
            "--push-peer-info unexpected",
            "push-peer-info\nup /tmp/evil",
            "push-peer-info\nplugin malicious.so",
            "push-peer-info\nconfig other.ovpn",
            "push-peer-info\nca /etc/secret",
        ] {
            assert!(
                validate_config(&format!("{CONFIG}{option}\n")).is_err(),
                "{option}"
            );
        }
    }
    #[test]
    fn compatibility_directives_are_preserved_in_exported_profiles() {
        for directive in [
            "ignore-unknown-option block-outside-dns",
            "--ignore-unknown-option block-outside-dns",
            "  ignore-unknown-option block-outside-dns register-dns  ",
            "ignore-unknown-option \"block-outside-dns\" # client compatibility",
            "ignore-unknown-option data-ciphers\nignore-unknown-option block-outside-dns",
        ] {
            let input =
                format!("\u{feff}{CONFIG}{directive}\nblock-outside-dns\n").replace('\n', "\r\n");
            let validated = validate_config(&input).unwrap();
            assert!(validated.contains(directive.trim()), "{directive}");
            assert!(validated.ends_with("remote-cert-tls server\n"));
        }
    }
    #[test]
    fn compatibility_directives_require_option_names() {
        for directive in [
            "ignore-unknown-option",
            "ignore-unknown-option # no names",
            "ignore-unknown-option \"\"",
            "ignore-unknown-option \"block-outside-dns register-dns\"",
            "ignore-unknown-option /tmp/profile",
            "ignore-unknown-option block-outside-dns *",
        ] {
            let error = validate_config(&format!("{CONFIG}{directive}\n")).unwrap_err();
            assert!(error.message.contains("needs one or more option names"));
        }
    }
    #[test]
    fn compatibility_directives_do_not_bypass_import_restrictions() {
        for directive in [
            "up /tmp/evil",
            "plugin malicious.so",
            "config other.ovpn",
            "ca /etc/secret",
            "auth-user-pass secret.txt",
            "dev tap",
            "management 0.0.0.0 1234",
            "unknown-option value",
            "tls-verify /tmp/check",
        ] {
            let option = directive.split_whitespace().next().unwrap();
            for input in [
                format!("{CONFIG}ignore-unknown-option {option}\n{directive}\n"),
                format!("{CONFIG}{directive}\nignore-unknown-option {option}\n"),
            ] {
                assert!(validate_config(&input).is_err(), "{directive}");
            }
        }
        let validated = validate_config(&format!(
            "{CONFIG}ignore-unknown-option verify-x509-name remote-cert-tls\nverify-x509-name vpn.example name\n"
        ))
        .unwrap();
        assert!(validated.contains("verify-x509-name vpn.example name\n"));
        assert!(validated.ends_with("remote-cert-tls server\n"));
    }
    #[test]
    fn unsafe_or_unsupported_profiles_are_rejected() {
        for option in [
            "up /tmp/evil",
            "plugin malicious.so",
            "config other.ovpn",
            "management 0.0.0.0 1234",
            "auth-user-pass secret.txt",
            "ca /etc/secret",
            "dev tap",
            "<connection>\nremote other 443\n</connection>",
        ] {
            assert!(
                validate_config(&format!("{CONFIG}{option}\n")).is_err(),
                "{option}"
            );
        }
        assert!(
            validate_config(CONFIG)
                .unwrap()
                .contains("remote-cert-tls server")
        );
        assert!(validate_config("remote example.invalid 1194\n").is_err());
    }
}
