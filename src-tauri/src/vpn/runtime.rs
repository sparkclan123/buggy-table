//! Bundled OpenVPN + private TCP/IP stack. No Docker or host network changes.
use super::engine;
use super::*;
use std::{
    net::SocketAddr,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use zeroize::Zeroizing;

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

fn runtime_error() -> AppError {
    failure(
        "VPN_RUNTIME",
        "The bundled VPN engine could not start. Reinstall the latest app for this computer; Docker is not required.",
    )
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Default)]
pub struct VpnRuntime {}
impl VpnRuntime {
    pub async fn info(&self) -> RuntimeInfo {
        match self.prepare().await {
            Ok(()) => RuntimeInfo {
                available: true,
                prepared: true,
                message: "Built-in OpenVPN ready. No Docker or additional installation needed."
                    .into(),
            },
            Err(error) => RuntimeInfo {
                available: false,
                prepared: false,
                message: error.message,
            },
        }
    }
    pub async fn prepare(&self) -> AppResult<()> {
        engine::verify().await
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
        let mut cmd = engine::command()?;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| runtime_error())?;
        let endpoint = listener.local_addr().map_err(|_| runtime_error())?;
        let token = Zeroizing::new(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ));
        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(feature = "vpn-integration")]
        if std::env::var_os("BUGGY_VPN_DIAGNOSTICS").is_some() {
            cmd.stderr(Stdio::inherit());
        }
        let mut child = cmd.spawn().map_err(|_| runtime_error())?;
        let mut input = child.stdin.take().ok_or_else(runtime_error)?;
        let output = child.stdout.take().ok_or_else(runtime_error)?;
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
            .map_err(|_| runtime_error())?,
        );
        bytes.push(b'\n');
        if !matches!(
            tokio::time::timeout(Duration::from_secs(55), input.write_all(&bytes)).await,
            Ok(Ok(()))
        ) {
            drop(input);
            drop(child);
            return Err(runtime_error());
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
        let native_port = Arc::new(AtomicU16::new(0));
        let watch_port = native_port.clone();
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
                            let Some(port) = event["port"].as_u64().and_then(|value| u16::try_from(value).ok()).filter(|port| *port > 0) else { break };
                            watch_port.store(port, Ordering::SeqCst);
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
                    exit = child.wait() => {
                        #[cfg(feature = "vpn-integration")]
                        if std::env::var_os("BUGGY_VPN_DIAGNOSTICS").is_some() { eprintln!("Native VPN exit: {exit:?}"); }
                        let _ = exit;
                        break
                    },
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
        let peer = SocketAddr::from(([127, 0, 0, 1], native_port.load(Ordering::SeqCst)));
        if peer.port() == 0 {
            return Err(runtime_error());
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
        // kill_on_drop and stdin EOF also terminate the owned native process.
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
        "VPN_TIMEOUT" => failure(
            "VPN_TIMEOUT",
            "OpenVPN did not connect before the timeout. Check the VPN server address, network and profile.",
        ),
        "VPN_RUNTIME" => runtime_error(),
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
