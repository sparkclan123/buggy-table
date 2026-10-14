# TLS test fixture

This public RSA private key is for loopback tests only. Never use it for a real
service. Tests generate short-lived certificates at runtime, avoiding expiring
fixtures and satisfying macOS certificate lifetime limits. RSA keeps native TLS
identity import compatible with macOS and Windows.
