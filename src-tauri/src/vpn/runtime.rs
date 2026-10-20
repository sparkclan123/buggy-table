//! Docker is only used when a VPN is enabled. No host networking or host mounts.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    net::SocketAddr,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{OnceCell, watch},
    task::JoinHandle,
};
use zeroize::Zeroizing;

const DOCKERFILE: &str = include_str!("../../vpn/Dockerfile");
const HELPER: &str = include_str!("../../vpn/helper.py");
const UP: &str = include_str!("../../vpn/up.py");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStatus {
    pub state: String,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub connected_at: Option<u64>,
    pub checked_at: Option<u64>,
    pub latency_ms: Option<u64>,
    pub message: String,
}
impl VpnStatus {
    pub fn direct() -> Self {
        Self {
            state: "direct".into(),
            profile_id: None,
            profile_name: None,
            connected_at: None,
            checked_at: None,
            latency_ms: None,
            message: "Using this device's normal network.".into(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub available: bool,
    pub prepared: bool,
    pub message: String,
}

#[derive(Clone)]
struct DockerCli {
    binary: PathBuf,
    endpoint: String,
}
impl DockerCli {
    async fn discover() -> AppResult<Self> {
        let mut paths = Vec::new();
        #[cfg(target_os = "macos")]
        paths.extend([
            PathBuf::from("/usr/local/bin/docker"),
            PathBuf::from("/Applications/Docker.app/Contents/Resources/bin/docker"),
        ]);
        #[cfg(target_os = "windows")]
        if let Some(programs) = std::env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(programs).join("Docker/Docker/resources/bin/docker.exe"));
        }
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path).map(|p| {
                p.join(if cfg!(windows) {
                    "docker.exe"
                } else {
                    "docker"
                })
            }));
        }
        let binary = paths
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(docker_error)?;
        let mut cmd = Command::new(&binary);
        cmd.args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ]);
        let endpoint = run_output(cmd, 8).await?;
        let endpoint = endpoint.trim().to_string();
        if !(endpoint.starts_with("unix:///") || endpoint.starts_with("npipe:////./pipe/")) {
            return Err(failure(
                "VPN_DOCKER_LOCAL",
                "Choose a local Docker Desktop context. Remote Docker engines are not supported for private VPN profiles.",
            ));
        }
        let cli = Self { binary, endpoint };
        let mut cmd = cli.command();
        cmd.args(["info", "--format", "{{.OSType}}"]);
        if run_output(cmd, 10).await?.trim() != "linux" {
            return Err(docker_error());
        }
        Ok(cli)
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("--host").arg(&self.endpoint);
        for name in [
            "DOCKER_HOST",
            "DOCKER_CONTEXT",
            "DOCKER_TLS_VERIFY",
            "DOCKER_CERT_PATH",
            "DOCKER_API_VERSION",
        ] {
            cmd.env_remove(name);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd
    }
    async fn image(&self) -> AppResult<String> {
        let mut cmd = self.command();
        cmd.args(["image", "inspect", "--format", "{{.Id}}", &image_tag()]);
        run_output(cmd, 8).await
    }
    async fn available(&self) -> AppResult<()> {
        let mut cmd = self.command();
        cmd.args(["info", "--format", "{{.OSType}}"]);
        if run_output(cmd, 10).await?.trim() != "linux" {
            return Err(docker_error());
        }
        Ok(())
    }
    async fn remove(&self, name: &str) {
        let mut cmd = self.command();
        cmd.args(["rm", "--force", name]);
        let _ = run_output(cmd, 10).await;
    }
}

fn image_tag() -> String {
    let mut hash = Sha256::new();
    for source in [DOCKERFILE, HELPER, UP] {
        hash.update(source.as_bytes());
    }
    format!("buggy-table-openvpn:{:x}", hash.finalize())
}
async fn run_output(mut cmd: Command, seconds: u64) -> AppResult<String> {
    cmd.kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let result = tokio::time::timeout(Duration::from_secs(seconds), cmd.output())
        .await
        .map_err(|_| docker_error())?
        .map_err(|_| docker_error())?;
    if !result.status.success() || result.stdout.len() > 64 * 1024 {
        return Err(docker_error());
    }
    String::from_utf8(result.stdout).map_err(|_| docker_error())
}
fn docker_error() -> AppError {
    failure(
        "VPN_DOCKER",
        "Open Docker Desktop with Linux containers, then retry. VPN uses only a local Docker engine; direct connections do not require Docker.",
    )
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Default)]
pub struct VpnRuntime {
    cli: OnceCell<DockerCli>,
    build: tokio::sync::Mutex<()>,
}
impl VpnRuntime {
    async fn cli(&self) -> AppResult<&DockerCli> {
        self.cli.get_or_try_init(DockerCli::discover).await
    }
    pub async fn info(&self) -> RuntimeInfo {
        match self.cli().await {
            Ok(cli) => {
                if let Err(e) = cli.available().await {
                    return RuntimeInfo {
                        available: false,
                        prepared: false,
                        message: e.message,
                    };
                }
                let prepared = cli.image().await.is_ok();
                RuntimeInfo {
                    available: true,
                    prepared,
                    message: if prepared {
                        "OpenVPN runtime ready."
                    } else {
                        "Prepare the OpenVPN runtime once. Docker Desktop must be running."
                    }
                    .into(),
                }
            }
            Err(e) => RuntimeInfo {
                available: false,
                prepared: false,
                message: e.message,
            },
        }
    }
    pub async fn prepare(&self) -> AppResult<()> {
        self.ensure_image().await.map(|_| ())
    }
    async fn ensure_image(&self) -> AppResult<(DockerCli, String)> {
        let _lock = self.build.lock().await;
        let cli = self.cli().await?.clone();
        if let Ok(id) = cli.image().await {
            return Ok((cli, id.trim().into()));
        }
        let context = tempfile::tempdir().map_err(|_| docker_error())?;
        for (name, text) in [
            ("Dockerfile", DOCKERFILE),
            ("helper.py", HELPER),
            ("up.py", UP),
        ] {
            std::fs::write(context.path().join(name), text).map_err(|_| docker_error())?;
        }
        let mut cmd = cli.command();
        cmd.args(["build", "--quiet", "--tag", &image_tag()])
            .arg(context.path());
        run_output(cmd, 240).await.map_err(|_| failure("VPN_PREPARE", "Could not prepare OpenVPN. Check Docker Desktop and internet access, then use Prepare in VPN settings."))?;
        let id = cli.image().await?;
        Ok((cli, id.trim().into()))
    }
    pub async fn start(
        &self,
        profile: VpnProfile,
        secrets: VpnSecrets,
        host: &str,
        port: u16,
    ) -> AppResult<Arc<Tunnel>> {
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(|c| {
                c.is_whitespace() || c.is_control() || matches!(c, '/' | '@' | ',' | '#' | '?')
            })
            || port == 0
        {
            return Err(failure(
                "VPN_DESTINATION",
                "Enter one database hostname/IP and valid port.",
            ));
        }
        if host
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback() || ip.is_unspecified() || ip.is_multicast())
        {
            return Err(failure(
                "VPN_DESTINATION",
                "For VPN connections, use the database's private hostname or IP, not localhost.",
            ));
        }
        let (cli, image) = self.ensure_image().await?;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| docker_error())?;
        let endpoint = listener.local_addr().map_err(|_| docker_error())?;
        let name = format!("buggy-table-vpn-{}", uuid::Uuid::new_v4().simple());
        let token = Zeroizing::new(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ));
        let mut cmd = cli.command();
        cmd.args([
            "run",
            "--rm",
            "--interactive",
            "--init",
            "--pull",
            "never",
            "--name",
            &name,
            "--label",
            "dev.tablepor.vpn=1",
            "--cap-drop",
            "ALL",
            "--cap-add",
            "NET_ADMIN",
            "--cap-add",
            "NET_RAW",
            "--device",
            "/dev/net/tun",
            "--security-opt",
            "no-new-privileges",
            "--read-only",
            "--tmpfs",
            "/run/vpn:rw,noexec,nosuid,nodev,size=8m",
            "--pids-limit",
            "48",
            "--memory",
            "128m",
            "--cpus",
            "1",
            "--log-driver",
            "none",
            "--publish",
            "127.0.0.1::17353",
            &image,
        ]);
        cmd.stdin(Stdio::piped());
        let mut child = cmd.spawn().map_err(|_| docker_error())?;
        let mut input = child.stdin.take().ok_or_else(docker_error)?;
        let output = child.stdout.take().ok_or_else(docker_error)?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Start<'a> {
            config: &'a str,
            username: &'a str,
            password: &'a str,
            key_password: &'a str,
            host: &'a str,
            port: u16,
            dns: &'a [String],
            token: &'a str,
        }
        let mut bytes = Zeroizing::new(
            serde_json::to_vec(&Start {
                config: &secrets.config,
                username: &profile.username,
                password: &secrets.password,
                key_password: &secrets.key_password,
                host,
                port,
                dns: &profile.dns,
                token: &token,
            })
            .map_err(|_| docker_error())?,
        );
        bytes.push(b'\n');
        if !matches!(
            tokio::time::timeout(Duration::from_secs(55), input.write_all(&bytes)).await,
            Ok(Ok(()))
        ) {
            drop(input);
            drop(child);
            cli.remove(&name).await;
            return Err(docker_error());
        }
        drop(bytes);
        drop(secrets);
        let alive = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(VpnStatus {
            state: "connecting".into(),
            profile_id: Some(profile.id.clone()),
            profile_name: Some(profile.name.clone()),
            connected_at: None,
            checked_at: None,
            latency_ms: None,
            message: "Connecting OpenVPN…".into(),
        }));
        let (updates, mut receiver) = watch::channel(false);
        let watch_alive = alive.clone();
        let watch_status = status.clone();
        let process = tokio::spawn(async move {
            let mut lines = BufReader::new(output).lines();
            let mut heartbeat = tokio::time::interval(Duration::from_secs(8));
            loop {
                tokio::select! {
                    line = lines.next_line() => {
                        let Ok(Some(line)) = line else { break };
                        if line.len() > 1024 { break; }
                        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                        if event["state"] == "ready" {
                            watch_alive.store(true, Ordering::SeqCst);
                            if let Ok(mut status) = watch_status.lock() { status.state = "ready".into(); status.connected_at = Some(now()); status.message = "VPN tunnel ready.".into(); }
                            let _ = updates.send(true);
                        } else if event["state"] == "disconnected" {
                            watch_alive.store(false, Ordering::SeqCst);
                            if let Ok(mut status) = watch_status.lock() { status.state = "disconnected".into(); status.message = vpn_error(event["code"].as_str().unwrap_or("VPN_DISCONNECTED")).message; }
                            let _ = updates.send(true);
                        }
                    }
                    _ = heartbeat.tick() => if input.write_all(b"keepalive\n").await.is_err() { break },
                    _ = child.wait() => break,
                }
            }
            watch_alive.store(false, Ordering::SeqCst);
            if let Ok(mut status) = watch_status.lock() {
                if status.state != "disconnected" {
                    status.message =
                        "VPN stopped. Reconnect VPN before using this database.".into();
                }
                status.state = "disconnected".into();
            }
            let _ = updates.send(true);
            drop(input);
        });
        let mut tunnel = Tunnel {
            profile,
            endpoint,
            peer: "127.0.0.1:1".parse().unwrap(),
            token,
            alive,
            status,
            cli,
            name,
            process,
            relay: None,
            probing: tokio::sync::Mutex::new(()),
        };
        let startup = tokio::time::timeout(Duration::from_secs(55), async {
            loop {
                let state = tunnel.status();
                if state.state == "ready" { break Ok(()); }
                if state.state == "disconnected" { break Err(failure("VPN_CONNECT", &state.message)); }
                receiver.changed().await.map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
            }
        }).await.map_err(|_| failure("VPN_TIMEOUT", "OpenVPN did not connect within 55 seconds. Check the profile, credentials and VPN server."))?;
        startup?;
        let mut cmd = tunnel.cli.command();
        cmd.args(["port", &tunnel.name, "17353/tcp"]);
        let address = run_output(cmd, 8).await?;
        let peer: SocketAddr = address.trim().parse().map_err(|_| docker_error())?;
        if !peer.ip().is_loopback() {
            return Err(docker_error());
        }
        tunnel.peer = peer;
        tunnel.probe().await?;
        let auth = tunnel.token.clone();
        let alive = tunnel.alive.clone();
        tunnel.relay = Some(tokio::spawn(async move {
            let mut jobs = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut stream, _)) = accepted else { break };
                        if !alive.load(Ordering::SeqCst) || jobs.len() >= 24 { continue; }
                        let token = auth.clone();
                        jobs.spawn(async move {
                            let _ = stream.set_nodelay(true);
                            if let Ok(mut remote) = authenticate(peer, &token, b'C').await { let _ = tokio::io::copy_bidirectional(&mut stream, &mut remote).await; }
                        });
                    }
                    _ = jobs.join_next(), if !jobs.is_empty() => {},
                }
            }
        }));
        Ok(Arc::new(tunnel))
    }
}

pub struct Tunnel {
    pub profile: VpnProfile,
    pub endpoint: SocketAddr,
    peer: SocketAddr,
    token: Zeroizing<String>,
    alive: Arc<AtomicBool>,
    status: Arc<Mutex<VpnStatus>>,
    cli: DockerCli,
    name: String,
    process: JoinHandle<()>,
    relay: Option<JoinHandle<()>>,
    probing: tokio::sync::Mutex<()>,
}
impl Tunnel {
    pub fn ensure_alive(&self) -> AppResult<()> {
        if self.alive.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(failure("VPN_DISCONNECTED", &self.status().message))
        }
    }
    pub fn status(&self) -> VpnStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| VpnStatus {
                state: "disconnected".into(),
                message: "VPN state unavailable.".into(),
                ..VpnStatus::direct()
            })
    }
    pub async fn probe(&self) -> AppResult<VpnStatus> {
        let _guard = self
            .probing
            .try_lock()
            .map_err(|_| failure("BUSY", "A VPN route check is already running."))?;
        self.ensure_alive()?;
        let check = tokio::time::timeout(Duration::from_secs(20), async {
            let mut socket = authenticate(self.peer, &self.token, b'P').await?;
            let mut line = String::new();
            BufReader::new(&mut socket)
                .take(32)
                .read_line(&mut line)
                .await
                .map_err(|_| vpn_error("VPN_DESTINATION"))?;
            line.trim()
                .parse::<u64>()
                .map_err(|_| vpn_error("VPN_DESTINATION"))
        })
        .await
        .unwrap_or_else(|_| Err(vpn_error("VPN_DESTINATION")));
        self.ensure_alive()?;
        let mut status = self
            .status
            .lock()
            .map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
        status.checked_at = Some(now());
        match check {
            Ok(ms) => {
                status.state = "ready".into();
                status.latency_ms = Some(ms);
                status.message = "VPN and database TCP route are reachable. SQL authentication is checked when connecting.".into();
                Ok(status.clone())
            }
            Err(error) => {
                status.state = "unreachable".into();
                status.latency_ms = None;
                status.message = error.message.clone();
                Err(error)
            }
        }
    }
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(relay) = &self.relay {
            relay.abort();
        }
        self.process.abort();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let cli = self.cli.clone();
            let name = self.name.clone();
            runtime.spawn(async move {
                cli.remove(&name).await;
            });
        }
        // stdin EOF / 30-second heartbeat deadline also removes orphan containers.
    }
}
async fn authenticate(peer: SocketAddr, token: &str, mode: u8) -> AppResult<TcpStream> {
    tokio::time::timeout(Duration::from_secs(18), async {
        let mut socket = TcpStream::connect(peer)
            .await
            .map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
        let _ = socket.set_nodelay(true);
        socket
            .write_all(token.as_bytes())
            .await
            .map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
        socket
            .write_all(&[b'\n', mode])
            .await
            .map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
        let mut answer = Vec::new();
        while answer.len() < 80 {
            let byte = socket
                .read_u8()
                .await
                .map_err(|_| vpn_error("VPN_DISCONNECTED"))?;
            if byte == b'\n' {
                break;
            }
            answer.push(byte);
        }
        if answer != b"OK" {
            return Err(vpn_error(
                std::str::from_utf8(&answer)
                    .unwrap_or("")
                    .strip_prefix("ERR ")
                    .unwrap_or("VPN_DESTINATION"),
            ));
        }
        Ok(socket)
    })
    .await
    .map_err(|_| vpn_error("VPN_DESTINATION"))?
}
fn vpn_error(code: &str) -> AppError {
    match code {
        "VPN_AUTH" => failure(
            "VPN_AUTH",
            "OpenVPN rejected authentication. Check VPN credentials; browser SSO and interactive challenge prompts are not supported.",
        ),
        "VPN_CERTIFICATE" => failure(
            "VPN_CERTIFICATE",
            "OpenVPN certificate verification failed. Check the VPN CA, server identity and certificate expiry.",
        ),
        "VPN_CONFIG" => failure(
            "VPN_CONFIG",
            "OpenVPN could not load this client profile. Check the inline certificates, key password and supported options.",
        ),
        "VPN_DNS_REQUIRED" => failure(
            "VPN_DNS_REQUIRED",
            "The VPN did not supply DNS. Add its private DNS IP in VPN settings or use the database's private IP.",
        ),
        "VPN_DNS" => failure(
            "VPN_DNS",
            "The database hostname could not be resolved through VPN DNS. No system DNS fallback was used.",
        ),
        "VPN_DESTINATION" => failure(
            "VPN_DESTINATION",
            "The VPN cannot reach the database TCP port. Check its private hostname/IP, VPN routes and server firewall.",
        ),
        _ => failure(
            "VPN_DISCONNECTED",
            "OpenVPN is disconnected. Reconnect VPN to continue; this connection will not fall back to the normal network.",
        ),
    }
}
