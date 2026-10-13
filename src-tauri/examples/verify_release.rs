//! Verify CI artifacts against the public key actually embedded in release builds.
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use std::{error::Error, fs, path::Path};

fn decoded(value: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(STANDARD.decode(value.trim())?)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("Pass the artifact directory")?;
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.release.conf.json"))?;
    let key = PublicKey::decode(&decoded(
        config["plugins"]["updater"]["pubkey"]
            .as_str()
            .ok_or("Missing public key")?,
    )?)?;
    let mut checked = 0;
    for entry in fs::read_dir(&directory)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sig") {
            continue;
        }
        let signature = Signature::decode(&decoded(&fs::read_to_string(&path)?)?)?;
        let artifact = path.with_extension("");
        let bytes = fs::read(Path::new(&artifact))?;
        key.verify(&bytes, &signature, true)?;
        let signed_version = signature
            .trusted_comment()
            .split('\t')
            .find_map(|field| field.strip_prefix("version:"));
        if signed_version != Some(env!("CARGO_PKG_VERSION")) {
            return Err("Signature version does not match this build".into());
        }
        checked += 1;
    }
    if checked == 0 {
        return Err("No signed packages found".into());
    }
    println!("Verified {checked} package(s), public key and signed version.");
    Ok(())
}
