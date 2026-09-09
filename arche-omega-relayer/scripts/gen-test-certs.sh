#!/usr/bin/env bash
# Generate a minimal CA + server + client cert chain for local mTLS tests.
set -euo pipefail

CERT_DIR="${1:-certs}"
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

echo "==> Generating test CA..."
openssl genrsa -out ca.key 2048 2>/dev/null
openssl req -x509 -new -nodes -key ca.key -sha256 -days 3650 \
  -subj "/CN=kette12-test-ca" -out ca.crt 2>/dev/null

echo "==> Generating server cert..."
openssl genrsa -out server.key 2048 2>/dev/null
openssl req -new -key server.key -subj "/CN=kette12-relay" -out server.csr 2>/dev/null
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt -days 825 -sha256 2>/dev/null

echo "==> Generating client cert (CN=clap-provider-1)..."
openssl genrsa -out client.key 2048 2>/dev/null
openssl req -new -key client.key -subj "/CN=clap-provider-1" -out client.csr 2>/dev/null
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out client.crt -days 825 -sha256 2>/dev/null

# PKCS#8 for rustls
openssl pkcs8 -topk8 -nocrypt -in server.key -out server.pkcs8 2>/dev/null
mv server.pkcs8 server.key

openssl pkcs8 -topk8 -nocrypt -in client.key -out client.pkcs8 2>/dev/null
mv client.pkcs8 client.key

rm -f server.csr client.csr ca.srl

echo "==> Done. Files in $CERT_DIR:"
ls -la
