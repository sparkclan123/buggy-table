//! Session-only settings shared by the MySQL and Redis adapters.
use crate::models::*;
use std::net::IpAddr;
use zeroize::Zeroizing;

pub struct RemoteProfile {
    pub engine: Engine,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Zeroizing<String>,
    pub tls: TlsMode,
    pub root_ca_pem: String,
}

impl RemoteProfile {
    pub fn new(r: ConnectRequest) -> AppResult<Self> {
        let password = Zeroizing::new(r.password);
        if (r.tls == TlsMode::Require && r.engine != Engine::Mysql)
            || (!r.root_ca_pem.is_empty()
                && (r.engine != Engine::Mysql || r.tls != TlsMode::VerifyFull))
        {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Custom CA certificates require verified TLS on PostgreSQL or MySQL. Redis supports verified TLS or loopback only.",
            ));
        }
        crate::postgres::parse_root_ca(&r.root_ca_pem)?;
        if r.manual_edits {
            return Err(AppError::policy(
                "MySQL and Redis connections are read-only in this release.",
            ));
        }
        if [&r.name, &r.host, &r.database]
            .iter()
            .any(|v| v.trim().is_empty() || v.len() > 253 || v.contains('\0'))
            || r.username.len() > 253
            || r.username.contains('\0')
            || password.len() > 4096
            || r.port == 0
            || r.host
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '/' | ',' | '@' | '?' | '#'))
        {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Enter one host, a valid port and database; connection fields must be at most 253 bytes.",
            ));
        }
        if r.tls == TlsMode::LocalOnly && !r.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        {
            return Err(AppError::new(
                "TLS_REQUIRED",
                "Unencrypted connections require a literal loopback IP. Remote hosts require verified TLS.",
            ));
        }
        if r.engine == Engine::Redis && !r.database.parse::<u16>().is_ok_and(|db| db <= 1023) {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "Redis database must be an integer from 0 to 1023.",
            ));
        }
        if r.engine == Engine::Mysql && r.username.is_empty() {
            return Err(AppError::new(
                "INVALID_CONNECTION",
                "MySQL requires a username.",
            ));
        }
        Ok(Self {
            engine: r.engine,
            name: r.name,
            host: r.host,
            port: r.port,
            database: r.database,
            username: r.username,
            password,
            tls: r.tls,
            root_ca_pem: r.root_ca_pem,
        })
    }

    pub fn info(&self, id: String, version: String, tables: Vec<TableInfo>) -> ConnectionInfo {
        ConnectionInfo {
            engine: self.engine,
            id,
            name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            database: self.database.clone(),
            username: self.username.clone(),
            schema: self.database.clone(),
            tls: self.tls,
            server_version: version,
            manual_edits: false,
            tables,
        }
    }
}
