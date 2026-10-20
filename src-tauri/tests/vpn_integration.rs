//! Run after `python3 scripts/vpn-fixtures.py start`. No production endpoints.
use std::{path::PathBuf, time::Instant};
use table_por::{
    models::*,
    service::Workbench,
    vpn::{VpnProfile, VpnSecrets, runtime::VpnRuntime, validate_config},
};
use uuid::Uuid;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/vpn-fixtures")
}
fn profile(index: u8) -> (VpnProfile, VpnSecrets) {
    (
        VpnProfile {
            id: Uuid::new_v4().to_string(),
            owner: Uuid::new_v4().to_string(),
            name: format!("Fixture {index}"),
            username: String::new(),
            dns: vec![],
            has_password: false,
            has_key_password: false,
        },
        VpnSecrets {
            config: validate_config(
                &std::fs::read_to_string(fixture().join(format!("client-{index}.ovpn"))).unwrap(),
            )
            .unwrap(),
            password: String::new(),
            key_password: String::new(),
        },
    )
}
fn request(host: &str) -> ConnectRequest {
    ConnectRequest {
        engine: Engine::Postgres,
        name: "Disposable VPN".into(),
        host: host.into(),
        port: 5432,
        database: "fixture".into(),
        username: "vpn_reader".into(),
        password: "disposable-vpn-test".into(),
        schema: "public".into(),
        tls: TlsMode::VerifyFull,
        root_ca_pem: std::fs::read_to_string(fixture().join("ca.crt")).unwrap(),
        manual_edits: false,
    }
}
fn query(id: &str) -> QueryRequest {
    QueryRequest {
        connection_id: id.into(),
        request_id: Uuid::new_v4().to_string(),
        sql: "SELECT id, label FROM public.items ORDER BY id".into(),
        explain: false,
        schema: Some("public".into()),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires disposable OpenVPN fixtures and local Docker Desktop"]
async fn isolated_vpns_keep_dns_tls_sessions_and_fail_closed() {
    let runtime = VpnRuntime::default();
    runtime.prepare().await.unwrap();
    let started = Instant::now();
    let (a, sa) = profile(0);
    let (b, sb) = profile(1);
    let owner = a.owner.clone();
    let route_a = runtime.start(a, sa, "vpn-db.test", 5432).await.unwrap();
    let route_b = runtime.start(b, sb, "vpn-db.test", 5432).await.unwrap();
    eprintln!(
        "Two independent VPN handshakes and DNS/TCP preflights: {:?}",
        started.elapsed()
    );
    assert_ne!(route_a.endpoint, route_b.endpoint);
    let workbench = Workbench::default();
    let a = workbench
        .connect_route(
            request("vpn-db.test"),
            Some(route_a.clone()),
            None,
            Some(owner.clone()),
        )
        .await
        .unwrap();
    let b = workbench
        .connect_route(request("vpn-db.test"), Some(route_b.clone()), None, None)
        .await
        .unwrap();
    assert_eq!(a.host, "vpn-db.test"); // Original hostname is kept for certificate checks.
    assert!(a.vpn.enabled && b.vpn.enabled);
    let qa = workbench.query(query(&a.id)).await.unwrap();
    let qb = workbench.query(query(&b.id)).await.unwrap();
    assert_eq!(qa.rows[0][1].as_deref(), Some("A"));
    assert_eq!(qb.rows[0][1].as_deref(), Some("B"));
    for _ in 0..5 {
        let result = workbench.query(query(&a.id)).await.unwrap();
        assert!(result.timings.unwrap().connection_reused);
        eprintln!("Reused VPN database query: {} ms", result.elapsed_ms);
    }
    assert_eq!(
        workbench.vpn_status(&a.id, true).await.unwrap().state,
        "ready"
    );
    assert!(
        workbench
            .check_owner(&a.id, &Uuid::new_v4().to_string())
            .await
            .is_err()
    );
    // Wrong DB identity is rejected even though the relay endpoint is loopback.
    let (wrong, secret) = profile(0);
    let wrong = runtime
        .start(wrong, secret, "10.77.0.1", 5432)
        .await
        .unwrap();
    let mut invalid = request("wrong-name.test");
    invalid.root_ca_pem = std::fs::read_to_string(fixture().join("ca.crt")).unwrap();
    assert!(
        workbench
            .connect_route(invalid, Some(wrong.clone()), None, None)
            .await
            .is_err()
    );
    drop(wrong);
    // Failed replacement preserves the old session and its network.
    let mut invalid = request("vpn-db.test");
    invalid.password = "wrong".into();
    assert!(
        workbench
            .connect_route(invalid, Some(route_b.clone()), Some(&b.id), None)
            .await
            .is_err()
    );
    assert!(workbench.query(query(&b.id)).await.is_ok());
    // Remove only the disposable VPN server. A must stop; B remains usable.
    let status = std::process::Command::new("docker")
        .args(["stop", "buggy-table-vpn-fixture-a"])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(workbench.query(query(&a.id)).await.is_err());
    assert!(workbench.query(query(&b.id)).await.is_ok());
    assert_ne!(
        workbench.vpn_status(&a.id, true).await.unwrap().state,
        "ready"
    );
    workbench.disconnect(&a.id).await.unwrap();
    workbench.disconnect(&b.id).await.unwrap();
    drop(route_a);
    drop(route_b);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}
