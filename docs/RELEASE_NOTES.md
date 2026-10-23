buggy-table 0.1.17 — Preview

Connect to PostgreSQL without entering a database name. Expand databases and schemas in the sidebar, then open the tables you need.

## Changes

- Make PostgreSQL's Default database optional. Discover connectable, non-template databases through an accessible maintenance database; if none is available, enter any database your account can access.
- Add compact +/− database and schema controls. Load schemas and tables on demand, search loaded names, and refresh individual schemas/databases without discarding open tabs or pending edits.
- Keep database identity with every table tab, query, count, index lookup and save. Identically named tables in different databases remain separate, including after reconnecting. Existing saved layouts migrate automatically.
- Reuse up to four idle PostgreSQL connections per server workspace. Switching back avoids another handshake while bounded eviction and disconnect close unused transports. Privileges remain checked for every operation.
- Retain the built-in, isolated OpenVPN engine introduced in 0.1.16. No Docker Desktop, separate OpenVPN client, network driver or runtime download is needed for VPN.

## Validation

Real PostgreSQL integration covers blank/default database connections, permission-filtered discovery, maintenance fallback, same-named tables across databases/schemas, quoted database names, reads/counts/indexes/overview/saves, strict-reader checks, connection reuse, eviction and cleanup. Chromium and WebKit tests cover lazy browsing, errors/retry, empty databases, independent drafts, saved tabs and both themes. Build provenance and final CI results accompany the installers in BUILD_INFO.txt.

## Install this Preview

Supports macOS 11 or newer, or Windows x64 with WebView2 (the installer supplies it if missing). Save/discard pending edits and close the app before installing over the old version. Keep the app-data folder to retain saved connections and encrypted VPN profiles. The Updates button opens GitHub downloads; this Preview does not install updates automatically.

| Platform | Installer |
|---|---|
| Mac Apple Silicon | `buggy-table_0.1.17_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.17_x64.dmg` |
| Windows x64 | `buggy-table_0.1.17_x64-setup.exe` |

Mac apps use ad-hoc signatures without Apple Developer ID/notarization; Windows builds have no Authenticode certificate. OS security prompts may still occur. Checksums and build provenance accompany the installers. Database and VPN performance depends on network latency, server workload and query design.
