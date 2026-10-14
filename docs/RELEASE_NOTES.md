buggy-table 0.1.1 — Preview

- Add an explicit PostgreSQL TLS mode that requires encryption without verifying the certificate/hostname, for compatibility with private/self-signed servers. Verified TLS remains the default. Neither TLS mode falls back to plaintext.
- Support session-only custom CA PEM bundles for verified PostgreSQL connections.
- Distinguish TLS, network and authentication errors with actionable messages.
- Keep PostgreSQL-only TLS options isolated from MySQL/Redis.

This preview has no Apple Developer ID/notarization or Windows Authenticode certificate, and its in-app updater is disabled. Install manually. Unverified TLS encrypts traffic but does not prevent server impersonation; choose it only for an endpoint you trust. Database contents and credentials are not included in update requests.
