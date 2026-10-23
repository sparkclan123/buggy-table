buggy-table 0.1.18 — Preview

Fix OpenVPN profile imports that failed with “Unsupported OpenVPN option: ignore-unknown-option”. Install this version and import the original .ovpn again; the compatibility directive does not need to be removed.

## Changes

- Accept and preserve `ignore-unknown-option`, including the common `ignore-unknown-option block-outside-dns` export, multiple option names and repeated directives.
- Validate option-name syntax with a specific error for missing or malformed names. Every actual profile directive still passes the app's existing compatibility checks; scripts, plugins and external files remain unsupported.
- Keep certificate identity verification active even when its option name appears in an ignore list. The bundled OpenVPN 3 engine processes recognized TLS options normally.
- Retain the built-in isolated VPN engine and PostgreSQL/MySQL browsing without a required default database. Users do not need Docker Desktop, a separate OpenVPN client or a runtime download.

## Validation

Regression coverage checks exported profile syntax, encrypted profile persistence, malformed input and attempts to bypass profile restrictions. Disposable UDP and TCP VPN fixtures now include this compatibility directive, exercising actual private DNS, database TLS, reused connections and certificate rejection. Build provenance and final CI results accompany the installers in BUILD_INFO.txt. Automated regression tests use synthetic profiles and disposable databases.

## Install this Preview

Supports macOS 11 or newer, or Windows x64 with WebView2 (the installer supplies it if missing). Save/discard pending edits and close the app before installing over the old version. Keep the app-data folder to retain saved connections and encrypted VPN profiles. The Updates button opens GitHub downloads; this Preview does not install updates automatically.

| Platform | Installer |
|---|---|
| Mac Apple Silicon | `buggy-table_0.1.18_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.18_x64.dmg` |
| Windows x64 | `buggy-table_0.1.18_x64-setup.exe` |

Mac apps use ad-hoc signatures without Apple Developer ID/notarization; Windows builds have no Authenticode certificate. OS security prompts may still occur. Checksums and build provenance accompany the installers. Database and VPN performance depends on network latency, server workload and query design.
