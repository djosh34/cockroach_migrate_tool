#!/usr/bin/env bash

set -euo pipefail

# Dependencies:
# - bash
# - envsubst
# - yq

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/setup_sql_common.sh
source "$script_dir/lib/setup_sql_common.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/generate-postgres-grants-sql.sh [--dry-run] [--output-dir DIR] <config.yml>

Generate PostgreSQL destination grants SQL files from a YAML config.
EOF
}

append_sorted_unique_lines() {
  local contents="$1"
  local -n target_ref="$2"

  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    target_ref+="$line"$'\n'
  done < <(printf '%s' "$contents" | sort -u)
}

main() {
  setup_sql::parse_common_args usage "$@"
  setup_sql::require_commands envsubst
  setup_sql::yaml_backend >/dev/null

  local config_path="$SETUP_SQL_INPUT_PATH"
  local mapping_count
  mapping_count="$(setup_sql::yaml_length "$config_path" 'mappings')"
  (( mapping_count > 0 )) || setup_sql::die "at least one mapping is required"

  local -a database_order=()
  declare -A database_header=()
  declare -A database_grants=()
  declare -A schema_grants=()
  declare -A table_grants=()

  local mapping_index mapping_id database runtime_role
  mapping_index=0

  while (( mapping_index < mapping_count )); do
    mapping_id="$(setup_sql::yaml_scalar "$config_path" "mappings.$mapping_index.id")"
    database="$(setup_sql::yaml_scalar "$config_path" "mappings.$mapping_index.destination.database")"
    runtime_role="$(setup_sql::yaml_scalar "$config_path" "mappings.$mapping_index.destination.runtime_role")"

    [[ -n "$mapping_id" ]] || setup_sql::die "missing required key: mappings[$mapping_index].id"
    [[ -n "$database" ]] || setup_sql::die "missing required key: mappings[$mapping_index].destination.database"
    [[ -n "$runtime_role" ]] || setup_sql::die "missing required key: mappings[$mapping_index].destination.runtime_role"

    local -a tables
    mapfile -t tables < <(setup_sql::yaml_list "$config_path" "mappings.$mapping_index.destination.tables")
    ((${#tables[@]} > 0)) || setup_sql::die "missing required key: mappings[$mapping_index].destination.tables"

    if [[ -z "${database_header[$database]+x}" ]]; then
      database_order+=("$database")
      database_header["$database"]="$(
        DATABASE="$database" envsubst <<'EOF'
-- PostgreSQL grants SQL
-- Destination database: ${DATABASE}
-- Helper schema: _cockroach_migration_tool
EOF
      )"
      database_header["$database"]+=$'\n\n'
    fi

    database_grants["$database"]+="GRANT CONNECT, CREATE ON DATABASE ${database} TO ${runtime_role};"$'\n'

    local table_name schema_name
    for table_name in "${tables[@]}"; do
      if [[ "$table_name" != *.* ]]; then
        setup_sql::die "invalid table reference for mappings[$mapping_index].destination.tables: $table_name"
      fi
      schema_name="${table_name%%.*}"
      schema_grants["$database"]+="GRANT USAGE ON SCHEMA ${schema_name} TO ${runtime_role};"$'\n'
      table_grants["$database"]+="GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ${table_name} TO ${runtime_role};"$'\n'
    done

    ((mapping_index += 1))
  done

  local combined_contents=''
  local file_contents database
  for database in "${database_order[@]}"; do
    file_contents="${database_header[$database]}"
    append_sorted_unique_lines "${database_grants[$database]-}" file_contents
    append_sorted_unique_lines "${schema_grants[$database]-}" file_contents
    append_sorted_unique_lines "${table_grants[$database]-}" file_contents
    setup_sql::emit_file \
      "$SETUP_SQL_OUTPUT_DIR/postgres-${database}-grants.sql" \
      "$file_contents"
    combined_contents+="$file_contents"
  done

  setup_sql::emit_file "$SETUP_SQL_OUTPUT_DIR/postgres-all-grants.sql" "$combined_contents"
}

main "$@"
