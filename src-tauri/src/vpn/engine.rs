//! Locate only our bundled sidecar, never an arbitrary program from PATH.
use super::failure;
use crate::models::AppResult;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::process::Command;

fn binary() -> AppResult<PathBuf> {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        let bundled = directory.join(format!("buggy-vpn{suffix}"));
        if bundled.is_file() {
            return Ok(bundled);
        }
    }
    #[cfg(any(debug_assertions, feature = "vpn-integration"))]
    {
        #[cfg(feature = "vpn-integration")]
        if let Some(path) = std::env::var_os("BUGGY_VPN_BINARY") {
            let path = PathBuf::from(path);
            if path.is_absolute() && path.is_file() {
                return Ok(path);
            }
        }
        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("buggy-vpn-{}{suffix}", env!("VPN_TARGET")));
        if development.is_file() {
            return Ok(development);
        }
    }
    Err(failure(
        "VPN_RUNTIME_MISSING",
        "The bundled VPN engine is missing. Reinstall the latest app for this computer. Docker is not required.",
    ))
}

pub(super) fn command() -> AppResult<Command> {
    let mut command = Command::new(binary()?);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command.kill_on_drop(true);
    Ok(command)
}

pub(super) async fn verify() -> AppResult<()> {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        command()?
            .arg("--self-test")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output(),
    )
    .await;
    if let Ok(Ok(output)) = result
        && output.status.success()
        && output.stdout.starts_with(b"buggy-vpn protocol 1;")
    {
        return Ok(());
    }
    Err(failure(
        "VPN_RUNTIME",
        "The bundled VPN engine could not run. Reinstall the latest app for this computer. Docker is not required.",
    ))
}
