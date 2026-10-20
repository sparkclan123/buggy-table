//! Docker setup commands contain only the bundled runtime, never VPN profiles.
use super::failure;
use crate::models::{AppError, AppResult};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 32 * 1024;

fn executable_path(binary: &Path, inherited: Option<&OsStr>) -> Option<OsString> {
    let canonical = std::fs::canonicalize(binary).ok();
    let mut paths = Vec::new();
    // Finder/LaunchServices may omit Docker Desktop's credential helpers from PATH.
    for parent in canonical
        .as_deref()
        .and_then(Path::parent)
        .into_iter()
        .chain(binary.parent())
    {
        if parent.is_absolute() && !paths.iter().any(|p| p == parent) {
            paths.push(parent.to_path_buf());
        }
    }
    if let Some(inherited) = inherited {
        for path in std::env::split_paths(inherited) {
            if path.is_absolute() && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    std::env::join_paths(paths).ok()
}

pub(super) fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    if let Some(path) = executable_path(binary, std::env::var_os("PATH").as_deref()) {
        cmd.env("PATH", path);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}

pub(super) async fn build_image(mut cmd: Command, context: &Path, tag: &str) -> AppResult<()> {
    // Use the selected local engine, regardless of the user's buildx/cloud builder.
    for name in [
        "BUILDX_BUILDER",
        "BUILDKIT_HOST",
        "BUILDX_CONFIG",
        "DOCKER_DEFAULT_PLATFORM",
    ] {
        cmd.env_remove(name);
    }
    cmd.env("DOCKER_BUILDKIT", "1")
        .args([
            "build",
            "--builder",
            "default",
            "--load",
            "--quiet",
            "--tag",
            tag,
        ])
        .arg(context);
    run_build(cmd, Duration::from_secs(600)).await
}

async fn read_tail(mut stream: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut tail = Vec::with_capacity(OUTPUT_LIMIT);
    let mut chunk = [0; 4096];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(tail);
        }
        let excess = (tail.len() + count).saturating_sub(OUTPUT_LIMIT);
        tail.drain(..excess);
        tail.extend_from_slice(&chunk[..count]);
    }
}

async fn run_build(mut cmd: Command, deadline: Duration) -> AppResult<()> {
    cmd.kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|_| {
        failure(
            "VPN_PREPARE_EXEC",
            "Could not start Docker's build tools. Reopen Docker Desktop and retry Prepare VPN.",
        )
    })?;
    let stdout = child.stdout.take().expect("piped build stdout");
    let stderr = child.stderr.take().expect("piped build stderr");
    let result = tokio::time::timeout(deadline, async {
        tokio::try_join!(child.wait(), read_tail(stdout), read_tail(stderr))
    })
    .await;
    match result {
        Ok(Ok((status, _, _))) if status.success() => Ok(()),
        Ok(Ok((status, stdout, stderr))) => {
            let output = format!(
                "{}\n{}",
                String::from_utf8_lossy(&stderr),
                String::from_utf8_lossy(&stdout)
            );
            Err(build_error(&output, status.code()))
        }
        Ok(Err(_)) => Err(failure(
            "VPN_PREPARE_IO",
            "Docker stopped responding during setup. Check Docker Desktop, then retry Prepare VPN.",
        )),
        Err(_) => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            Err(failure(
                "VPN_PREPARE_TIMEOUT",
                "OpenVPN setup exceeded 10 minutes. Check Docker Desktop's network or proxy, then retry Prepare VPN; downloaded layers can be reused.",
            ))
        }
    }
}

fn build_error(output: &str, exit: Option<i32>) -> AppError {
    let text = output.to_ascii_lowercase();
    // Return actionable categories, never registry credentials, proxy URLs or raw logs.
    let (code, message) = if text.contains("docker-credential-")
        && (text.contains("not found") || text.contains("executable file"))
    {
        (
            "VPN_PREPARE_HELPER",
            "Docker's credential helper could not be found. Repair or update Docker Desktop, reopen this app, then retry Prepare VPN.",
        )
    } else if text.contains("error getting credentials")
        || text.contains("keychain")
        || text.contains("credentials store")
    {
        (
            "VPN_PREPARE_CREDENTIALS",
            "Docker could not read its saved registry credentials. Unlock the system password store or sign in again in Docker Desktop, then retry Prepare VPN.",
        )
    } else if text.contains("x509:")
        || text.contains("certificate verify failed")
        || text.contains("certificate verification failed")
        || text.contains("certificate signed by unknown authority")
    {
        (
            "VPN_PREPARE_CERTIFICATE",
            "Docker could not verify a download certificate. Check the Mac/PC clock and Docker Desktop's trusted proxy or company CA, then retry Prepare VPN.",
        )
    } else if text.contains("toomanyrequests") || text.contains("rate limit") {
        (
            "VPN_PREPARE_RATE_LIMIT",
            "The container registry limited downloads. Sign in to Docker Desktop or wait before retrying Prepare VPN.",
        )
    } else if text.contains("no space left") || text.contains("disk quota exceeded") {
        (
            "VPN_PREPARE_DISK",
            "Docker has insufficient disk space to prepare OpenVPN. Free space or increase Docker Desktop's disk limit, then retry.",
        )
    } else if text.contains("cannot connect to the docker daemon")
        || text.contains("is the docker daemon running")
    {
        (
            "VPN_DOCKER",
            "Docker Desktop stopped or is still starting. Wait for its engine to be ready, then retry Prepare VPN.",
        )
    } else if text.contains("buildx")
        && (text.contains("missing")
            || text.contains("not found")
            || text.contains("broken")
            || text.contains("not a docker command"))
        || text.contains("unknown flag: --builder")
    {
        (
            "VPN_PREPARE_BUILDER",
            "Docker's Buildx component is missing or outdated. Update Docker Desktop, then retry Prepare VPN.",
        )
    } else if text.contains("no such host")
        || text.contains("network is unreachable")
        || text.contains("connection refused")
        || text.contains("connection reset")
        || text.contains("timeout")
        || text.contains("timed out")
        || text.contains("temporary failure")
        || text.contains("temporary error")
        || text.contains("i/o timeout")
        || text.contains("failed to fetch")
        || text.contains("unable to fetch")
    {
        (
            "VPN_PREPARE_NETWORK",
            "Docker could not download the OpenVPN runtime. Check Docker Desktop's internet, DNS or proxy settings, then retry Prepare VPN.",
        )
    } else if text.contains("unauthorized") || text.contains("authentication required") {
        (
            "VPN_PREPARE_REGISTRY",
            "The container registry rejected Docker's credentials. Sign in again in Docker Desktop, then retry Prepare VPN.",
        )
    } else {
        return failure(
            "VPN_PREPARE_BUILD",
            &format!(
                "Docker could not build the OpenVPN runtime (exit {}). Open Docker Desktop → Builds for the failed build details, then retry Prepare VPN.",
                exit.map_or_else(|| "unknown".into(), |code| code.to_string())
            ),
        );
    };
    failure(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_helpers_are_found_without_a_shell_path() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("docker");
        std::fs::write(&binary, "").unwrap();
        let original = std::env::join_paths([std::env::temp_dir()]).unwrap();
        let result = executable_path(&binary, Some(&original)).unwrap();
        let paths: Vec<_> = std::env::split_paths(&result).collect();
        assert_eq!(paths[0], directory.path().canonicalize().unwrap());
        assert!(paths.contains(&std::env::temp_dir()));
    }

    #[test]
    fn setup_errors_explain_the_cause_without_echoing_credentials() {
        for (output, code) in [
            (
                "exec: docker-credential-desktop: executable file not found",
                "VPN_PREPARE_HELPER",
            ),
            (
                "error getting credentials: keychain locked",
                "VPN_PREPARE_CREDENTIALS",
            ),
            (
                "x509: certificate signed by unknown authority",
                "VPN_PREPARE_CERTIFICATE",
            ),
            (
                "toomanyrequests: rate limit exceeded",
                "VPN_PREPARE_RATE_LIMIT",
            ),
            ("write: no space left on device", "VPN_PREPARE_DISK"),
            ("Cannot connect to the Docker daemon", "VPN_DOCKER"),
            (
                "buildx component is missing or broken",
                "VPN_PREPARE_BUILDER",
            ),
            ("lookup registry: no such host", "VPN_PREPARE_NETWORK"),
            ("failed to fetch package index", "VPN_PREPARE_NETWORK"),
            (
                "unauthorized: authentication required",
                "VPN_PREPARE_REGISTRY",
            ),
            ("unrecognized build failure", "VPN_PREPARE_BUILD"),
        ] {
            let error = build_error(
                &format!("{output} https://user:SECRET_CANARY@proxy.invalid"),
                Some(1),
            );
            assert_eq!(error.code, code);
            assert!(!error.message.contains("SECRET_CANARY"));
            assert!(!error.message.contains("proxy.invalid"));
        }
    }

    #[tokio::test]
    async fn build_output_is_drained_and_bounded() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let send = tokio::spawn(async move {
            writer
                .write_all(&vec![b'x'; OUTPUT_LIMIT * 4])
                .await
                .unwrap();
            writer.write_all(b"final error").await.unwrap();
        });
        let tail = read_tail(reader).await.unwrap();
        send.await.unwrap();
        assert_eq!(tail.len(), OUTPUT_LIMIT);
        assert!(tail.ends_with(b"final error"));
    }

    #[test]
    fn build_process_fixture() {
        let Ok(mode) = std::env::var("BUGGY_VPN_BUILD_TEST_MODE") else {
            return;
        };
        match mode.as_str() {
            "failure" => {
                eprintln!("lookup registry: no such host; SECRET_CANARY");
                std::process::exit(9);
            }
            "timeout" => std::thread::sleep(Duration::from_secs(10)),
            _ => println!("runtime image built"),
        }
    }

    fn fixture_command(mode: &str) -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "vpn::docker::tests::build_process_fixture",
            "--nocapture",
        ])
        .env("BUGGY_VPN_BUILD_TEST_MODE", mode);
        cmd
    }

    #[tokio::test]
    async fn build_process_reports_success_failure_and_timeout() {
        run_build(fixture_command("success"), Duration::from_secs(5))
            .await
            .unwrap();
        let error = run_build(fixture_command("failure"), Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(error.code, "VPN_PREPARE_NETWORK");
        assert!(!error.message.contains("SECRET_CANARY"));
        let started = std::time::Instant::now();
        let error = run_build(fixture_command("timeout"), Duration::from_millis(100))
            .await
            .unwrap_err();
        assert_eq!(error.code, "VPN_PREPARE_TIMEOUT");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
