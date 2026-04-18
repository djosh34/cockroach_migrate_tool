#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd -- "${SCRIPT_DIR}/.." && pwd)
MOLT_IMAGE=${MOLT_IMAGE:-cockroachdb/molt@sha256:abe3c90bc42556ad6713cba207b971e6d55dbd54211b53cfcf27cdc14d49e358}

run_crdb_sql_file() {
  local file_path=$1
  docker compose exec -T cockroach \
    cockroach sql --insecure --host=localhost:26257 < "${file_path}"
}

run_pg_sql_file() {
  local file_path=$1
  docker compose exec -T postgres \
    psql -v ON_ERROR_STOP=1 -U postgres -d postgres < "${file_path}"
}

wait_for_cockroach() {
  local attempt
  for attempt in $(seq 1 60); do
    if docker compose exec -T cockroach \
      cockroach sql --insecure --host=localhost:26257 -e "select 1" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "CockroachDB did not become ready in time" >&2
  return 1
}

wait_for_postgres() {
  local attempt
  for attempt in $(seq 1 60); do
    if docker compose exec -T postgres \
      psql -U postgres -d postgres -c "select 1" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "PostgreSQL did not become ready in time" >&2
  return 1
}

run_verify() {
  local output_path=$1
  local exit_code_path=$2
  local cockroach_container
  cockroach_container=$(docker compose ps -q cockroach)

  set +e
  docker run --rm --network "container:${cockroach_container}" "${MOLT_IMAGE}" verify \
    --allow-tls-mode-disable \
    --source 'postgres://postgres:postgres@postgres:5432/verify_demo?sslmode=disable' \
    --target 'postgresql://root@127.0.0.1:26257/verify_demo?sslmode=disable' \
    --schema-filter 'public' \
    --table-filter 'customers|products|orders|order_items' \
    > "${output_path}" 2>&1
  local exit_code=$?
  set -e

  printf '%s\n' "${exit_code}" > "${exit_code_path}"
}

main() {
  cd "${ROOT_DIR}"

  mkdir -p output/molt-verify
  rm -f output/molt-verify/*

  docker compose down -v --remove-orphans >/dev/null 2>&1 || true
  docker compose up -d cockroach postgres

  wait_for_cockroach
  wait_for_postgres

  docker run --rm "${MOLT_IMAGE}" --version > output/molt-verify/version.txt

  run_crdb_sql_file "${ROOT_DIR}/sql/10_verify_crdb_schema.sql"
  run_crdb_sql_file "${ROOT_DIR}/sql/11_verify_crdb_seed.sql"

  run_pg_sql_file "${ROOT_DIR}/sql/12_verify_pg_schema.sql"
  run_pg_sql_file "${ROOT_DIR}/sql/13_verify_pg_seed.sql"

  run_verify \
    "${ROOT_DIR}/output/molt-verify/baseline.log" \
    "${ROOT_DIR}/output/molt-verify/baseline.exit_code"

  run_crdb_sql_file "${ROOT_DIR}/sql/14_verify_crdb_mismatch.sql"

  run_verify \
    "${ROOT_DIR}/output/molt-verify/mismatch.log" \
    "${ROOT_DIR}/output/molt-verify/mismatch.exit_code"

  python3 "${ROOT_DIR}/scripts/summarize-molt-verify.py" > "${ROOT_DIR}/output/molt-verify/summary.pretty.json"
}

main "$@"
