buggy-table 0.1.3 — Preview

- Add saved connection bookmarks, pin/recent/search/group filters and explicit Reconnect after restarting. Optional passwords stay in Mac login Keychain / Windows Local Credential Manager.
- Keep PostgreSQL/MySQL/Redis sessions and table tabs separate when switching; preserve unsaved cells and Redis inspection state.
- Add searchable PostgreSQL schema selection; bind browse/SQL/index/save to the owning table schema.
- Restore only table identities on startup and limit clean result caches; no query text, row data or drafts written to connection settings.
- Remove the PostgreSQL 17-only gate. Test PostgreSQL 15, 16, 17 and 18 with connection, browse, SQL, index/status, save/rollback, cancellation and TLS coverage.
- Allow newer PostgreSQL majors to attempt connection with an untested-compatibility notice; retain all safety checks.
- Exclude PostgreSQL 18 virtual generated columns from approved table scope; protect stored generated and identity columns from edits.
- Add an explicit PostgreSQL TLS mode that requires encryption without verifying the certificate/hostname, for compatibility with private/self-signed servers. Verified TLS remains the default. Neither TLS mode falls back to plaintext.
- Support optional locally saved custom CA PEM bundles for verified PostgreSQL connections.
- Distinguish TLS, network and authentication errors with actionable messages.
- Keep PostgreSQL-only TLS options isolated from MySQL/Redis.

This preview has no Apple Developer ID/notarization or Windows Authenticode certificate, and its in-app updater is disabled. Install manually. Unverified TLS encrypts traffic but does not prevent server impersonation; choose it only for an endpoint you trust. Database contents and credentials are not included in update requests.
