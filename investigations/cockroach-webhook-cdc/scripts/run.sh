#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd -- "${SCRIPT_DIR}/.." && pwd)

run_sql_file() {
  local file_path=$1
  docker compose exec -T cockroach \
    cockroach sql --insecure --host=localhost:26257 < "${file_path}"
}

run_sql_capture() {
  local output_path=$1
  local sql=$2
  docker compose exec -T cockroach \
    cockroach sql --insecure --host=localhost:26257 --format=table -e "${sql}" \
    > "${output_path}"
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

wait_for_receiver() {
  local attempt
  for attempt in $(seq 1 60); do
    if curl --silent --show-error --fail \
      --cacert "${ROOT_DIR}/certs/ca.crt" \
      "https://localhost:8443/healthz" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "Receiver did not become ready in time" >&2
  return 1
}

sql_scalar() {
  local sql=$1
  docker compose exec -T cockroach \
    cockroach sql --insecure --host=localhost:26257 --format=tsv -e "${sql}" \
    | tail -n 1
}

main() {
  cd "${ROOT_DIR}"

  mkdir -p output/requests output/sql
  rm -f output/requests/*.json output/sql/* output/summary.json
  rm -f certs/ca.crt certs/ca.key certs/ca.srl certs/server.crt certs/server.csr certs/server.key certs/server.cnf

  "${SCRIPT_DIR}/generate-certs.sh"

  docker compose down -v --remove-orphans >/dev/null 2>&1 || true
  docker compose up -d --build

  wait_for_cockroach
  wait_for_receiver

  run_sql_file "${ROOT_DIR}/sql/00_schema.sql"
  run_sql_file "${ROOT_DIR}/sql/01_seed.sql"
  docker compose exec -T cockroach \
    cockroach sql --insecure --host=localhost:26257 -e \
    "SET CLUSTER SETTING kv.rangefeed.enabled = true;"

  run_sql_capture "${ROOT_DIR}/output/sql/01_row_counts.txt" \
    "USE demo_cdc; SELECT 'customers' AS table_name, count(*) AS row_count FROM customers UNION ALL SELECT 'products', count(*) FROM products UNION ALL SELECT 'orders', count(*) FROM orders UNION ALL SELECT 'order_items', count(*) FROM order_items ORDER BY table_name;"

  local ca_cert_base64
  ca_cert_base64=$(base64 -w 0 < "${ROOT_DIR}/certs/ca.crt")

  local ca_cert_urlencoded
  ca_cert_urlencoded=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.stdin.read().strip(), safe=""))' <<< "${ca_cert_base64}")

  local live_sink_uri
  live_sink_uri="webhook-https://host.docker.internal:8443/enriched-live?ca_cert=${ca_cert_urlencoded}"

  local snapshot_sink_uri
  snapshot_sink_uri="webhook-https://host.docker.internal:8443/snapshot-only?ca_cert=${ca_cert_urlencoded}"

  local source_sink_uri
  source_sink_uri="webhook-https://host.docker.internal:8443/enriched-source?ca_cert=${ca_cert_urlencoded}"

  local live_job_id
  live_job_id=$(
    docker compose exec -T cockroach \
      cockroach sql --insecure --host=localhost:26257 --format=tsv -e "
        USE demo_cdc;
        CREATE CHANGEFEED FOR TABLE customers, products, orders, order_items
        INTO '${live_sink_uri}'
        WITH
          initial_scan = 'yes',
          envelope = 'enriched',
          diff,
          updated,
          mvcc_timestamp,
          resolved = '5s',
          min_checkpoint_frequency = '5s',
          webhook_sink_config = '{\"Flush\":{\"Messages\":25,\"Frequency\":\"2s\"},\"Retry\":{\"Max\":3,\"Backoff\":\"1s\"}}';
      " | tail -n 1
  )

  printf '%s\n' "${live_job_id}" > "${ROOT_DIR}/output/sql/live_job_id.txt"
  sleep 10

  run_sql_file "${ROOT_DIR}/sql/02_live_mutations.sql"
  sleep 8

  local snapshot_job_id
  snapshot_job_id=$(
    docker compose exec -T cockroach \
      cockroach sql --insecure --host=localhost:26257 --format=tsv -e "
        USE demo_cdc;
        CREATE CHANGEFEED FOR TABLE customers, products, orders, order_items
        INTO '${snapshot_sink_uri}'
        WITH
          initial_scan = 'only',
          envelope = 'enriched',
          webhook_sink_config = '{\"Flush\":{\"Messages\":25,\"Frequency\":\"2s\"}}';
      " | tail -n 1
  )

  printf '%s\n' "${snapshot_job_id}" > "${ROOT_DIR}/output/sql/snapshot_job_id.txt"
  sleep 10

  run_sql_file "${ROOT_DIR}/sql/03_post_snapshot_mutation.sql"
  sleep 8

  local source_job_id
  source_job_id=$(
    docker compose exec -T cockroach \
      cockroach sql --insecure --host=localhost:26257 --format=tsv -e "
        USE demo_cdc;
        CREATE CHANGEFEED FOR TABLE customers
        INTO '${source_sink_uri}'
        WITH
          initial_scan = 'no',
          envelope = 'enriched',
          enriched_properties = 'source',
          updated,
          mvcc_timestamp,
          webhook_sink_config = '{\"Flush\":{\"Messages\":1,\"Frequency\":\"1s\"}}';
      " | tail -n 1
  )

  printf '%s\n' "${source_job_id}" > "${ROOT_DIR}/output/sql/source_job_id.txt"
  sleep 5

  run_sql_file "${ROOT_DIR}/sql/04_source_probe_mutation.sql"
  sleep 8

  run_sql_capture "${ROOT_DIR}/output/sql/02_show_changefeed_jobs.txt" \
    "SELECT job_id, status, description, high_water_timestamp, running_status FROM [SHOW CHANGEFEED JOBS] ORDER BY job_id;"

  docker compose logs receiver > "${ROOT_DIR}/output/sql/03_receiver_logs.txt"

  python3 "${ROOT_DIR}/scripts/summarize.py" > "${ROOT_DIR}/output/sql/04_summary_pretty.json"
}

main "$@"
