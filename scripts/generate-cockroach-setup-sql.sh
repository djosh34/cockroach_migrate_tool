#!/usr/bin/env bash

set -euo pipefail

# Dependencies:
# - bash
# - envsubst
# - yq
# - python3
# - base64

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/setup_sql_common.sh
source "$script_dir/lib/setup_sql_common.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/generate-cockroach-setup-sql.sh [--dry-run] [--output-dir DIR] <config.yml>

Generate CockroachDB source setup SQL files from a YAML config.
EOF
}

percent_encode_file_base64() {
  local file_path="$1"

  base64 --wrap=0 "$file_path" | python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.stdin.read(), safe=""), end="")'
}

main() {
  setup_sql::parse_common_args usage "$@"
  setup_sql::require_commands envsubst base64
  setup_sql::yaml_backend >/dev/null

  local config_path="$SETUP_SQL_INPUT_PATH"
  local config_dir
  config_dir="$(cd "$(dirname "$config_path")" && pwd)"

  local cockroach_url webhook_base_url ca_cert_path resolved_interval mapping_count
  cockroach_url="$(setup_sql::yaml_scalar "$config_path" 'cockroach.url')"
  webhook_base_url="$(setup_sql::yaml_scalar "$config_path" 'webhook.base_url')"
  ca_cert_path="$(setup_sql::yaml_scalar "$config_path" 'webhook.ca_cert_path')"
  resolved_interval="$(setup_sql::yaml_scalar "$config_path" 'webhook.resolved')"
  mapping_count="$(setup_sql::yaml_length "$config_path" 'mappings')"

  [[ -n "$cockroach_url" ]] || setup_sql::die "missing required key: cockroach.url"
  [[ -n "$webhook_base_url" ]] || setup_sql::die "missing required key: webhook.base_url"
  [[ -n "$ca_cert_path" ]] || setup_sql::die "missing required key: webhook.ca_cert_path"
  [[ -n "$resolved_interval" ]] || setup_sql::die "missing required key: webhook.resolved"
  (( mapping_count > 0 )) || setup_sql::die "at least one mapping is required"

  webhook_base_url="$(setup_sql::trim_trailing_slashes "$webhook_base_url")"
  ca_cert_path="$(setup_sql::resolve_path "$config_dir" "$ca_cert_path")"
  [[ -f "$ca_cert_path" ]] || setup_sql::die "ca cert file not found: $ca_cert_path"

  local ca_cert_query_value
  ca_cert_query_value="$(percent_encode_file_base64 "$ca_cert_path")"

  local -a database_order=()
  declare -A database_contents=()
  declare -A database_has_mapping=()

  local mapping_index mapping_id database selected_tables changefeed_tables sql_contents
  mapping_index=0

  while (( mapping_index < mapping_count )); do
    mapping_id="$(setup_sql::yaml_scalar "$config_path" "mappings.$mapping_index.id")"
    database="$(setup_sql::yaml_scalar "$config_path" "mappings.$mapping_index.source.database")"

    [[ -n "$mapping_id" ]] || setup_sql::die "missing required key: mappings[$mapping_index].id"
    [[ -n "$database" ]] || setup_sql::die "missing required key: mappings[$mapping_index].source.database"

    local -a tables
    mapfile -t tables < <(setup_sql::yaml_list "$config_path" "mappings.$mapping_index.source.tables")
    ((${#tables[@]} > 0)) || setup_sql::die "missing required key: mappings[$mapping_index].source.tables"

    selected_tables=''
    changefeed_tables=''
    local table_name
    for table_name in "${tables[@]}"; do
      if [[ -n "$selected_tables" ]]; then
        selected_tables+=", "
      fi
      selected_tables+="$table_name"
      if [[ -n "$changefeed_tables" ]]; then
        changefeed_tables+=", "
      fi
      changefeed_tables+="${database}.${table_name}"
    done

    sql_contents="$(
      COCKROACH_URL="$cockroach_url" \
      MAPPING_ID="$mapping_id" \
      SELECTED_TABLES="$selected_tables" \
      CHANGEFEED_TABLES="$changefeed_tables" \
      WEBHOOK_BASE_URL="$webhook_base_url" \
      CA_CERT_QUERY_VALUE="$ca_cert_query_value" \
      RESOLVED_INTERVAL="$resolved_interval" \
        envsubst <<'EOF'
-- Mapping: ${MAPPING_ID}
-- Selected tables: ${SELECTED_TABLES}
-- Replace __CHANGEFEED_CURSOR__ below with the decimal cursor returned above before running the CREATE CHANGEFEED statement.
CREATE CHANGEFEED FOR TABLE ${CHANGEFEED_TABLES} INTO 'webhook-${WEBHOOK_BASE_URL}/ingest/${MAPPING_ID}?ca_cert=${CA_CERT_QUERY_VALUE}' WITH cursor = '__CHANGEFEED_CURSOR__', initial_scan = 'yes', envelope = 'enriched', resolved = '${RESOLVED_INTERVAL}';
EOF
    )"
    sql_contents+=$'\n'

    if [[ -z "${database_contents[$database]+x}" ]]; then
      database_order+=("$database")
      database_contents["$database"]="$(
        COCKROACH_URL="$cockroach_url" \
        DATABASE="$database" \
          envsubst <<'EOF'
-- Source bootstrap SQL
-- Cockroach URL: ${COCKROACH_URL}
-- Apply each statement with a Cockroach SQL client against the source cluster.
-- Capture the cursor once, then replace __CHANGEFEED_CURSOR__ in the CREATE CHANGEFEED statements below.

SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Source database: ${DATABASE}
USE ${DATABASE};
SELECT cluster_logical_timestamp() AS changefeed_cursor;
EOF
      )"
      database_contents["$database"]+=$'\n\n'
      database_has_mapping["$database"]='false'
    fi

    if [[ "${database_has_mapping[$database]}" == 'true' ]]; then
      database_contents["$database"]+=$'\n'
    fi
    database_contents["$database"]+="$sql_contents"
    database_has_mapping["$database"]='true'

    ((mapping_index += 1))
  done

  local combined_contents=''
  for database in "${database_order[@]}"; do
    setup_sql::emit_file \
      "$SETUP_SQL_OUTPUT_DIR/cockroach-${database}-setup.sql" \
      "${database_contents[$database]}"
    combined_contents+="${database_contents[$database]}"
  done
  setup_sql::emit_file "$SETUP_SQL_OUTPUT_DIR/cockroach-all-setup.sql" "$combined_contents"
}

main "$@"
