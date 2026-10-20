buggy-table 0.1.15 — Preview

Reduce repeated MySQL connection overhead and allow independent VPN connections to start concurrently.

## Changes

- Reuse one MySQL connection per workspace and up to 32 prepared statements, avoiding repeated authentication/TLS/session setup for ordinary reads.
- Keep every MySQL session read-only with autocommit. Reads see newly committed data without retaining transactions, snapshots or metadata locks between operations. Table metadata and permissions are still checked on every table read; result rows are not cached.
- Reuse connections after complete pages, including the pagination lookahead row. Discard incomplete results, failed reads and cancelled connections. Recover a disconnected cached transport during fixed catalog checks before executing user SQL; never automatically replay user execution.
- Keep exact counts on a dedicated connection so their shorter timeout does not change subsequent reads.
- Show total MySQL backend duration with connection, checks/schema, query/transfer and cleanup stages. Previous MySQL timings excluded connection and metadata setup, so the displayed numbers were not directly comparable with PostgreSQL.
- Allow independent VPN startups to overlap while protecting profile edits/removal from launch races. Already-connected database workspaces retain their existing tunnels; changing a VPN profile still establishes and validates a replacement.
- Retain optional MySQL database selection, lazy database navigation, per-connection OpenVPN settings and existing TLS verification behavior.

## Performance and validation

A disposable two-row benchmark with 20 ms relay delay in each direction measured 30 warm reads per operation. MySQL median total backend time decreased from 454 ms to 96 ms for SQL and from 440 ms to 95 ms for browsing (78–79% less time). This measures client connection overhead on local Docker fixtures, not production query speed, large-table throughput or a guarantee that one engine is faster than another. Results, reproduction commands and limitations are in [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.15/docs/VALIDATION.md).

Local regression coverage includes connection reuse, read-only enforcement with root, committed-data freshness, metadata lock release, permission revocation, pagination/byte limits, cancellation, stale connections, count isolation, multiple databases and TLS. Real two-network OpenVPN tests cover alternating/concurrent reads, private DNS, verified database TLS and independent tunnel failure. CI and installer checks are recorded in BUILD_INFO.txt.

## VPN requirements and limits

Install and start **Docker Desktop with Linux containers** and use a local Docker context. First use downloads/builds the runtime and may take several minutes; Prepare in VPN settings can do this in advance. Later sessions reuse the image. No Docker installation is needed for connections with VPN disabled.

Supported: OpenVPN TUN profiles with inline CA/certificates/keys, username/password authentication and encrypted private-key passwords. Use the database's private hostname/IP; add VPN DNS IPs if the server does not supply them. TAP, external certificate/key files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA/challenges are not supported. Disconnect before editing/removing a profile currently in use. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.15/docs/CONNECTIONS.md).

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.15/docs/VALIDATION.md). Browser cases use IPC doubles; real isolated OpenVPN/PostgreSQL fixtures verify separate networks, private DNS, TLS identity and failure isolation. External user VPN profiles and native Windows VPN operation have not been independently verified. MySQL and Redis retain their existing read-only policies.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.15_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.15_x64.dmg` |
| Windows x64 | `buggy-table_0.1.15_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
