#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd -- "${SCRIPT_DIR}/.." && pwd)
CERT_DIR="${ROOT_DIR}/certs"

mkdir -p "${CERT_DIR}"

if [[ -f "${CERT_DIR}/ca.crt" && -f "${CERT_DIR}/server.crt" && -f "${CERT_DIR}/server.key" ]]; then
  exit 0
fi

openssl req \
  -x509 \
  -newkey rsa:2048 \
  -days 365 \
  -nodes \
  -keyout "${CERT_DIR}/ca.key" \
  -out "${CERT_DIR}/ca.crt" \
  -subj "/CN=cdc-investigation-ca"

cat > "${CERT_DIR}/server.cnf" <<'EOF'
[req]
distinguished_name = dn
prompt = no
req_extensions = req_ext

[dn]
CN = receiver

[req_ext]
subjectAltName = @alt_names

[alt_names]
DNS.1 = receiver
DNS.2 = localhost
DNS.3 = host.docker.internal
IP.1 = 127.0.0.1
EOF

openssl req \
  -newkey rsa:2048 \
  -nodes \
  -keyout "${CERT_DIR}/server.key" \
  -out "${CERT_DIR}/server.csr" \
  -config "${CERT_DIR}/server.cnf"

openssl x509 \
  -req \
  -days 365 \
  -in "${CERT_DIR}/server.csr" \
  -CA "${CERT_DIR}/ca.crt" \
  -CAkey "${CERT_DIR}/ca.key" \
  -CAcreateserial \
  -out "${CERT_DIR}/server.crt" \
  -extensions req_ext \
  -extfile "${CERT_DIR}/server.cnf"
