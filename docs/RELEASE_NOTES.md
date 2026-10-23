buggy-table 0.1.16 — Preview

OpenVPN now comes with the app. Connect each database through its own VPN without installing Docker Desktop, OpenVPN, a network driver or an administrator helper.

## Changes

- Replace the Docker client runtime with a bundled native OpenVPN 3 engine and a private lwIP TCP/IP stack for each connection. The app does not create OS tunnel interfaces or change system routing/DNS.
- Remove first-use runtime downloads and the Docker preparation step. VPN settings show the built-in engine's readiness; existing encrypted profiles and defaults remain available.
- Keep independent tunnels and database sessions alive when switching workspaces. Private DNS, overlapping VPN IP addresses, fixed-destination forwarding and original-hostname database TLS verification remain isolated per connection.
- Stop the owned VPN process when its session ends or the app exits. VPN and DNS failures do not fall back to the device network.
- Use asynchronous forwarding with bounded buffers and TCP backpressure for large results. Package the engine's dependencies statically and include their source/license notices.

## Validation

Disposable local tests cover UDP and TCP OpenVPN, two separate networks with overlapping private IPs/DNS, certificate and username/password authentication, encrypted private keys, IPv4/IPv6, large TLS results, repeated/concurrent reads, reconnect replacement, independent tunnel failure and parent-exit cleanup. Negative cases reject incorrect VPN/DB certificate identities, wrong passwords, unauthorized relay access, missing VPN DNS and unreachable names. Memory/undefined-behavior sanitizer runs and installer checks are recorded in [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.16/docs/VALIDATION.md) and the release BUILD_INFO.txt.

These are disposable fixtures, not the user's production VPN or a universal latency guarantee. Windows installers run the bundled engine self-test; external VPN interoperability on every Windows network has not been independently verified.

## VPN profile support

Use OpenVPN TUN profiles with inline CA/certificates/keys. Username/password and encrypted private keys are supported. TAP, external files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA are not supported. Older server options/ciphers can differ between OpenVPN 2 and 3; unsupported configurations fail explicitly. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.16/docs/CONNECTIONS.md).

## Installation

Requires macOS 11 or newer, or Windows x64 with WebView2 (the installer supplies it if missing). Download the matching installer and replace the old app after saving/discarding pending edits and closing it. Retain the existing app-data folder to keep saved connections and encrypted VPN profiles. The Updates button opens GitHub downloads; this Preview does not install updates automatically.

| Platform | Installer |
|---|---|
| Mac Apple Silicon | `buggy-table_0.1.16_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.16_x64.dmg` |
| Windows x64 | `buggy-table_0.1.16_x64-setup.exe` |

Mac apps use ad-hoc signatures without Apple Developer ID/notarization; Windows builds have no Authenticode certificate. OS security prompts may therefore still occur. Checksums and build provenance accompany the installers.
