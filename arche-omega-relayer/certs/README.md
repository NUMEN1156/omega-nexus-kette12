# Certificates for mTLS

Place the following PEM files here (or adjust paths in `RelayConfig`):

- `server.crt` — Server certificate
- `server.key` — Server private key (PKCS#8)
- `ca.crt`     — CA certificate that signed the client certificates

The relay requires a valid client certificate signed by this CA (mTLS).
The `node_id` is extracted from the client certificate's Common Name (CN) or first DNS SAN.
