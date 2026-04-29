#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW_DIR="${ROOT_DIR}/raw"
HELM_DIR="${ROOT_DIR}/helm/verify-container"
LOCAL_OVERLAY_DIR="${ROOT_DIR}/.local-verification/raw-overlay"
CONFIG_DIR="${RAW_DIR}/config"
SECRETS_DIR="${RAW_DIR}/secrets"
WORK_DIR="${ROOT_DIR}/.local-work"

CRDB_NAME="verify-k8s-crdb"
PG_NAME="verify-k8s-postgres"
VERIFY_IMAGE_REPOSITORY="${VERIFY_IMAGE_REPOSITORY:-verify-image}"
VERIFY_IMAGE_TAG="${VERIFY_IMAGE_TAG:-k8s-local}"
POSTGRES_SUPERUSER_PASSWORD="${POSTGRES_SUPERUSER_PASSWORD:-postgres}"
POSTGRES_VERIFY_USERNAME="${POSTGRES_VERIFY_USERNAME:-verify_target}"
POSTGRES_VERIFY_PASSWORD="${POSTGRES_VERIFY_PASSWORD:-verify-target-password}"

mkdir -p "${LOCAL_OVERLAY_DIR}" "${CONFIG_DIR}" "${SECRETS_DIR}" "${WORK_DIR}/cockroach-certs" "${WORK_DIR}/postgres-tls"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_command docker
require_command openssl

cleanup_container() {
  local name="$1"
  if docker ps -a --format '{{.Names}}' | grep -Fxq "${name}"; then
    docker rm -f "${name}" >/dev/null
  fi
}

cleanup_container "${CRDB_NAME}"
cleanup_container "${PG_NAME}"

rm -f \
  "${WORK_DIR}/cockroach-certs/"* \
  "${WORK_DIR}/postgres-tls/"* \
  "${CONFIG_DIR}/verify-service.yml" \
  "${SECRETS_DIR}/source-username" \
  "${SECRETS_DIR}/destination-username" \
  "${SECRETS_DIR}/destination-password" \
  "${SECRETS_DIR}/source-ca.crt" \
  "${SECRETS_DIR}/source-client.crt" \
  "${SECRETS_DIR}/source-client.key" \
  "${SECRETS_DIR}/destination-ca.crt" \
  "${LOCAL_OVERLAY_DIR}/kustomization.yaml" \
  "${HELM_DIR}/values.local.yaml"

docker network inspect kind >/dev/null 2>&1 || {
  echo "docker network 'kind' does not exist; create the kind cluster first" >&2
  exit 1
}

docker run --rm -v "${WORK_DIR}/cockroach-certs:/certs" cockroachdb/cockroach:v26.1.2 \
  cert create-ca --certs-dir=/certs --ca-key=/certs/ca.key >/dev/null
docker run --rm -v "${WORK_DIR}/cockroach-certs:/certs" cockroachdb/cockroach:v26.1.2 \
  cert create-node localhost 127.0.0.1 "${CRDB_NAME}" --certs-dir=/certs --ca-key=/certs/ca.key >/dev/null
docker run --rm -v "${WORK_DIR}/cockroach-certs:/certs" cockroachdb/cockroach:v26.1.2 \
  cert create-client root --certs-dir=/certs --ca-key=/certs/ca.key >/dev/null

cat > "${WORK_DIR}/postgres-tls/server.cnf" <<'EOF_SERVER_CNF'
[req]
distinguished_name = dn
prompt = no
req_extensions = req_ext

[dn]
CN = verify-k8s-postgres

[req_ext]
subjectAltName = @alt_names

[alt_names]
DNS.1 = verify-k8s-postgres
DNS.2 = localhost
IP.1 = 127.0.0.1
EOF_SERVER_CNF

openssl req -x509 -newkey rsa:2048 -days 365 -nodes \
  -keyout "${WORK_DIR}/postgres-tls/ca.key" \
  -out "${WORK_DIR}/postgres-tls/ca.crt" \
  -subj "/CN=verify-k8s-postgres-ca" >/dev/null 2>&1

openssl req -newkey rsa:2048 -nodes \
  -keyout "${WORK_DIR}/postgres-tls/server.key" \
  -out "${WORK_DIR}/postgres-tls/server.csr" \
  -config "${WORK_DIR}/postgres-tls/server.cnf" >/dev/null 2>&1

openssl x509 -req -days 365 \
  -in "${WORK_DIR}/postgres-tls/server.csr" \
  -CA "${WORK_DIR}/postgres-tls/ca.crt" \
  -CAkey "${WORK_DIR}/postgres-tls/ca.key" \
  -CAcreateserial \
  -out "${WORK_DIR}/postgres-tls/server.crt" \
  -extensions req_ext \
  -extfile "${WORK_DIR}/postgres-tls/server.cnf" >/dev/null 2>&1

chmod 600 "${WORK_DIR}/postgres-tls/server.key"

docker run -d --name "${CRDB_NAME}" --network kind \
  -v "${WORK_DIR}/cockroach-certs:/cockroach-certs" \
  cockroachdb/cockroach:v26.1.2 start \
  --certs-dir=/cockroach-certs \
  --store=/cockroach/cockroach-data \
  --listen-addr=0.0.0.0:26357 \
  --advertise-addr="${CRDB_NAME}:26357" \
  --sql-addr=0.0.0.0:26257 \
  --advertise-sql-addr="${CRDB_NAME}:26257" \
  --http-addr=0.0.0.0:8080 \
  --join="${CRDB_NAME}:26357" >/dev/null

docker run -d --name "${PG_NAME}" --network kind \
  -e POSTGRES_PASSWORD="${POSTGRES_SUPERUSER_PASSWORD}" \
  -v "${WORK_DIR}/postgres-tls:/tls:ro" \
  postgres:16 \
  -c ssl=on \
  -c ssl_cert_file=/tls/server.crt \
  -c ssl_key_file=/tls/server.key \
  -c logging_collector=off >/dev/null

crdb_initialized=false
for _ in $(seq 1 60); do
  init_output="$(docker exec "${CRDB_NAME}" cockroach init --certs-dir=/cockroach-certs --host="${CRDB_NAME}:26357" 2>&1 || true)"
  if [[ "${init_output}" == *"Cluster successfully initialized"* ]] || [[ "${init_output}" == *"cluster has already been initialized"* ]]; then
    crdb_initialized=true
    break
  fi
  sleep 1
done

if [[ "${crdb_initialized}" != "true" ]]; then
  docker logs "${CRDB_NAME}" >&2 || true
  echo "cockroach did not initialize" >&2
  exit 1
fi

crdb_sql_ready=false
for _ in $(seq 1 60); do
  if docker exec "${CRDB_NAME}" cockroach sql --certs-dir=/cockroach-certs --host="${CRDB_NAME}:26257" -e 'select 1' >/dev/null 2>&1; then
    crdb_sql_ready=true
    break
  fi
  sleep 1
done

if [[ "${crdb_sql_ready}" != "true" ]]; then
  docker logs "${CRDB_NAME}" >&2 || true
  echo "cockroach SQL endpoint did not become ready" >&2
  exit 1
fi

pg_ready=false
for _ in $(seq 1 60); do
  if docker exec "${PG_NAME}" pg_isready -U postgres -d postgres >/dev/null 2>&1; then
    pg_ready=true
    break
  fi
  sleep 1
done

if [[ "${pg_ready}" != "true" ]]; then
  docker logs "${PG_NAME}" >&2 || true
  echo "postgres did not become ready" >&2
  exit 1
fi

docker exec "${CRDB_NAME}" cockroach sql --certs-dir=/cockroach-certs --host="${CRDB_NAME}:26257" -e "
CREATE DATABASE IF NOT EXISTS appdb;
USE appdb;
CREATE TABLE IF NOT EXISTS accounts (
  id INT PRIMARY KEY,
  owner STRING NOT NULL,
  balance INT NOT NULL
);
UPSERT INTO accounts (id, owner, balance) VALUES
  (1, 'alice', 10),
  (2, 'bob', 20);
" >/dev/null

docker exec -e PGPASSWORD="${POSTGRES_SUPERUSER_PASSWORD}" "${PG_NAME}" psql -U postgres -d postgres -v ON_ERROR_STOP=1 -c "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${POSTGRES_VERIFY_USERNAME}') THEN
    EXECUTE format('CREATE ROLE %I LOGIN PASSWORD %L', '${POSTGRES_VERIFY_USERNAME}', '${POSTGRES_VERIFY_PASSWORD}');
  END IF;
END
\$\$;
" >/dev/null

if ! docker exec -e PGPASSWORD="${POSTGRES_SUPERUSER_PASSWORD}" "${PG_NAME}" \
  psql -U postgres -d postgres -t -A -c "SELECT 1 FROM pg_database WHERE datname = 'appdb'" \
  | grep -Fxq '1'; then
  docker exec -e PGPASSWORD="${POSTGRES_SUPERUSER_PASSWORD}" "${PG_NAME}" \
    psql -U postgres -d postgres -v ON_ERROR_STOP=1 \
    -c "CREATE DATABASE appdb OWNER ${POSTGRES_VERIFY_USERNAME};" >/dev/null
fi

docker exec -e PGPASSWORD="${POSTGRES_SUPERUSER_PASSWORD}" "${PG_NAME}" psql -U postgres -d appdb -v ON_ERROR_STOP=1 -c "
SET ROLE ${POSTGRES_VERIFY_USERNAME};
CREATE TABLE IF NOT EXISTS accounts (
  id INTEGER PRIMARY KEY,
  owner TEXT NOT NULL,
  balance INTEGER NOT NULL
);
INSERT INTO accounts (id, owner, balance) VALUES
  (1, 'alice', 10),
  (2, 'bob', 20)
ON CONFLICT (id) DO UPDATE
SET owner = EXCLUDED.owner,
    balance = EXCLUDED.balance;
" >/dev/null

CRDB_IP="$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "${CRDB_NAME}")"
PG_IP="$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "${PG_NAME}")"

cp "${WORK_DIR}/cockroach-certs/ca.crt" "${SECRETS_DIR}/source-ca.crt"
cp "${WORK_DIR}/cockroach-certs/client.root.crt" "${SECRETS_DIR}/source-client.crt"
cp "${WORK_DIR}/cockroach-certs/client.root.key" "${SECRETS_DIR}/source-client.key"
cp "${WORK_DIR}/postgres-tls/ca.crt" "${SECRETS_DIR}/destination-ca.crt"

printf 'root\n' > "${SECRETS_DIR}/source-username"
printf '%s\n' "${POSTGRES_VERIFY_USERNAME}" > "${SECRETS_DIR}/destination-username"
printf '%s\n' "${POSTGRES_VERIFY_PASSWORD}" > "${SECRETS_DIR}/destination-password"

cat > "${CONFIG_DIR}/verify-service.yml" <<EOF_VERIFY_CONFIG
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
verify:
  source:
    host: ${CRDB_IP}
    port: 26257
    username:
      secret_file: /config/secrets/source-username
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    host: ${PG_IP}
    port: 5432
    username:
      secret_file: /config/secrets/destination-username
    password:
      secret_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: appdb
      destination_database: appdb
EOF_VERIFY_CONFIG

cat > "${LOCAL_OVERLAY_DIR}/kustomization.yaml" <<EOF_LOCAL_KUSTOMIZATION
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

resources:
  - ../../raw

images:
  - name: ghcr.io/example/verify-image
    newName: ${VERIFY_IMAGE_REPOSITORY}
    newTag: ${VERIFY_IMAGE_TAG}
EOF_LOCAL_KUSTOMIZATION

indent_file() {
  local path="$1"
  sed 's/^/      /' "${path}"
}

cat > "${HELM_DIR}/values.local.yaml" <<EOF_HELM_VALUES
image:
  repository: ${VERIFY_IMAGE_REPOSITORY}
  tag: ${VERIFY_IMAGE_TAG}
  pullPolicy: IfNotPresent

service:
  type: ClusterIP
  port: 8443

resources:
  requests:
    cpu: 100m
    memory: 128Mi
  limits:
    cpu: 500m
    memory: 256Mi

config:
  source:
    host: ${CRDB_IP}
    port: 26257
    database: appdb
  destination:
    host: ${PG_IP}
    port: 5432
    database: appdb
  databaseMappingName: app

secrets:
  credentials:
    sourceUsername: root
    destinationUsername: ${POSTGRES_VERIFY_USERNAME}
    destinationPassword: ${POSTGRES_VERIFY_PASSWORD}
  tls:
    sourceCaPem: |
$(indent_file "${SECRETS_DIR}/source-ca.crt")
    sourceClientCertPem: |
$(indent_file "${SECRETS_DIR}/source-client.crt")
    sourceClientKeyPem: |
$(indent_file "${SECRETS_DIR}/source-client.key")
    destinationCaPem: |
$(indent_file "${SECRETS_DIR}/destination-ca.crt")
EOF_HELM_VALUES

cat <<EOF_SUMMARY
Local verification inputs are ready.

External database endpoints:
- CockroachDB: ${CRDB_IP}:26257
- PostgreSQL: ${PG_IP}:5432

Generated files:
- ${CONFIG_DIR}/verify-service.yml
- ${SECRETS_DIR}/source-username
- ${SECRETS_DIR}/destination-username
- ${SECRETS_DIR}/destination-password
- ${SECRETS_DIR}/source-ca.crt
- ${SECRETS_DIR}/source-client.crt
- ${SECRETS_DIR}/source-client.key
- ${SECRETS_DIR}/destination-ca.crt
- ${LOCAL_OVERLAY_DIR}/kustomization.yaml
- ${HELM_DIR}/values.local.yaml

Use these image coordinates when loading the verify image into kind:
- repository: ${VERIFY_IMAGE_REPOSITORY}
- tag: ${VERIFY_IMAGE_TAG}
EOF_SUMMARY
