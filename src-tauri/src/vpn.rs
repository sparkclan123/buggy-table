//! Isolated, device-local OpenVPN profiles. Secrets never enter bookmarks or logs.
pub mod runtime;
pub mod store;

use crate::models::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnSelection {
    pub enabled: bool,
    pub profile_id: Option<String>,
}
impl VpnSelection {
    pub fn validate(&self) -> AppResult<()> {
        if self
            .profile_id
            .as_ref()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
            || (self.enabled && self.profile_id.is_none())
        {
            return Err(failure(
                "VPN_PROFILE",
                "Choose a default VPN profile first.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnProfile {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub username: String,
    pub dns: Vec<String>,
    pub has_password: bool,
    pub has_key_password: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnEdit {
    pub id: Option<String>,
    pub owner: String,
    pub name: String,
    /// None preserves the previous secret/configuration. An empty password clears it.
    pub config: Option<String>,
    pub username: String,
    pub password: Option<String>,
    pub key_password: Option<String>,
    pub dns: Vec<String>,
}
impl Drop for VpnEdit {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.config.zeroize();
        self.password.zeroize();
        self.key_password.zeroize();
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnSecrets {
    pub config: String,
    pub password: String,
    pub key_password: String,
}
impl Drop for VpnSecrets {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.config.zeroize();
        self.password.zeroize();
        self.key_password.zeroize();
    }
}

pub fn failure(code: &'static str, message: &str) -> AppError {
    AppError::new(code, message)
}

/// A deliberately bounded client profile format. OpenVPN files can otherwise
/// execute programs, load plugins, include arbitrary files or replace management.
/// Imported scripts are never executed, even inside the isolated container.
pub fn validate_config(config: &str) -> AppResult<String> {
    if config.len() > 512 * 1024 || config.contains('\0') {
        return Err(failure(
            "VPN_CONFIG",
            "OpenVPN profiles must be at most 512 KiB.",
        ));
    }
    let mut block: Option<String> = None;
    let mut remote = false;
    let mut ca = false;
    let mut output = String::new();
    let allowed = [
        "client",
        "tls-client",
        "dev",
        "dev-type",
        "proto",
        "remote",
        "remote-random",
        "remote-random-hostname",
        "nobind",
        "persist-key",
        "persist-tun",
        "resolv-retry",
        "connect-retry",
        "connect-retry-max",
        "connect-timeout",
        "server-poll-timeout",
        "auth",
        "auth-user-pass",
        "auth-nocache",
        // Client metadata used by some servers; no scripts or file access.
        "push-peer-info",
        "cipher",
        "data-ciphers",
        "data-ciphers-fallback",
        "remote-cert-tls",
        "remote-cert-ku",
        "remote-cert-eku",
        "verify-x509-name",
        "peer-fingerprint",
        "tls-version-min",
        "tls-version-max",
        "tls-cipher",
        "tls-ciphersuites",
        "tls-timeout",
        "reneg-sec",
        "reneg-bytes",
        "reneg-pkts",
        "hand-window",
        "tran-window",
        "key-direction",
        "verb",
        "mute",
        "mute-replay-warnings",
        "explicit-exit-notify",
        "keepalive",
        "ping",
        "ping-restart",
        "ping-exit",
        "tun-mtu",
        "tun-mtu-extra",
        "link-mtu",
        "mssfix",
        "sndbuf",
        "rcvbuf",
        "tcp-nodelay",
        "fast-io",
        "float",
        "route",
        "route-ipv6",
        "route-metric",
        "route-gateway",
        "redirect-gateway",
        "redirect-private",
        "dhcp-option",
        "pull",
        "pull-filter",
        "route-delay",
        "allow-compression",
        "compress",
        "comp-lzo",
        "disable-dco",
        "block-outside-dns",
        "register-dns",
        "auth-retry",
        "tls-exit",
    ];
    for raw in config.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if let Some(ref tag) = block {
            output.push_str(raw);
            output.push('\n');
            if line == format!("</{tag}>") {
                block = None;
            } else if line.starts_with('<') {
                return Err(failure(
                    "VPN_CONFIG",
                    "Invalid inline certificate/key block.",
                ));
            }
            continue;
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('<') {
            let tag = line.trim_start_matches('<').trim_end_matches('>');
            if ![
                "ca",
                "cert",
                "key",
                "tls-auth",
                "tls-crypt",
                "tls-crypt-v2",
                "extra-certs",
            ]
            .contains(&tag)
                || line != format!("<{tag}>")
            {
                return Err(failure(
                    "VPN_CONFIG",
                    "Use a client .ovpn with inline CA, certificates and keys. Connection blocks and external file references are not supported.",
                ));
            }
            ca |= tag == "ca";
            block = Some(tag.to_string());
        } else {
            if line.ends_with('\\') || line.len() > 4096 {
                return Err(failure(
                    "VPN_CONFIG",
                    "Multiline OpenVPN directives are not supported.",
                ));
            }
            let words = shlex::split(line)
                .ok_or_else(|| failure("VPN_CONFIG", "Invalid OpenVPN directive quoting."))?;
            let Some(first) = words.first() else { continue };
            let option = first.strip_prefix("--").unwrap_or(first.as_str());
            if !allowed.contains(&option) {
                return Err(failure(
                    "VPN_CONFIG",
                    &format!(
                        "Unsupported OpenVPN option: {}. This option is not supported by this app. Check the VPN profile compatibility guide.",
                        option.chars().take(60).collect::<String>()
                    ),
                ));
            }
            if option == "push-peer-info" && words.len() != 1 {
                return Err(failure(
                    "VPN_CONFIG",
                    "OpenVPN option push-peer-info does not accept arguments.",
                ));
            }
            if option == "auth-user-pass" && words.len() != 1 {
                return Err(failure(
                    "VPN_CONFIG",
                    "Enter VPN credentials in the form, not an auth file.",
                ));
            }
            if option == "dev" && !words.get(1).is_some_and(|s| s.starts_with("tun"))
                || option == "dev-type" && words.get(1).map(String::as_str) != Some("tun")
            {
                return Err(failure(
                    "VPN_CONFIG",
                    "Only OpenVPN TUN profiles are supported; TAP/bridged profiles are not.",
                ));
            }
            if option == "remote" {
                if words.len() < 2 || words.len() > 4 || words[1].starts_with('-') {
                    return Err(failure("VPN_CONFIG", "Invalid OpenVPN remote endpoint."));
                }
                remote = true;
            }
            if option == "peer-fingerprint" {
                ca = true;
            }
            // The application owns timeouts, interface, scripts and diagnostics.
            if [
                "dev",
                "dev-type",
                "verb",
                "mute",
                "persist-tun",
                "auth-nocache",
                "auth-retry",
                "connect-retry-max",
                "connect-timeout",
                "resolv-retry",
                "ping",
                "ping-restart",
                "ping-exit",
                "keepalive",
                "disable-dco",
                "block-outside-dns",
                "register-dns",
            ]
            .contains(&option)
            {
                continue;
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    if block.is_some() || !remote || !ca {
        return Err(failure(
            "VPN_CONFIG",
            "A complete client profile needs a remote server and inline CA (or peer fingerprint).",
        ));
    }
    // Server identity is required even if the imported profile omitted this directive.
    output.push_str("remote-cert-tls server\n");
    Ok(output)
}

pub fn validate_meta(owner: &str, name: &str, username: &str, dns: &[String]) -> AppResult<()> {
    if uuid::Uuid::parse_str(owner).is_err()
        || name.trim().is_empty()
        || name.len() > 80
        || name.chars().any(char::is_control)
        || username.len() > 253
        || username.chars().any(char::is_control)
        || dns.len() > 3
        || dns.iter().any(|s| {
            !s.parse::<IpAddr>()
                .is_ok_and(|ip| !ip.is_loopback() && !ip.is_unspecified() && !ip.is_multicast())
        })
    {
        return Err(failure(
            "VPN_PROFILE",
            "Use a short name, a valid connection ID and up to three VPN DNS IP addresses.",
        ));
    }
    Ok(())
}
