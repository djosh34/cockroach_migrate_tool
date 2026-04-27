#!/usr/bin/env bash

set -euo pipefail

setup_sql::die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

setup_sql::repo_root() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  printf '%s\n' "$script_dir"
}

setup_sql::require_commands() {
  local command_name
  for command_name in "$@"; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
      setup_sql::die "required command not found: $command_name"
    fi
  done
}

setup_sql::resolve_path() {
  local base_dir="$1"
  local path_value="$2"

  if [[ "$path_value" = /* ]]; then
    printf '%s\n' "$path_value"
    return
  fi

  printf '%s/%s\n' "$base_dir" "$path_value"
}

setup_sql::trim_trailing_slashes() {
  local value="$1"

  while [[ "$value" == */ ]]; do
    value="${value%/}"
  done

  printf '%s\n' "$value"
}

setup_sql::parse_common_args() {
  local usage_function="$1"
  shift

  SETUP_SQL_INPUT_PATH=''
  SETUP_SQL_OUTPUT_DIR='output'
  SETUP_SQL_DRY_RUN='false'

  while (($# > 0)); do
    case "$1" in
      --help|-h)
        "$usage_function"
        exit 0
        ;;
      --dry-run)
        SETUP_SQL_DRY_RUN='true'
        shift
        ;;
      --output-dir)
        (($# >= 2)) || setup_sql::die "--output-dir requires a value"
        SETUP_SQL_OUTPUT_DIR="$2"
        shift 2
        ;;
      --*)
        setup_sql::die "unknown option: $1"
        ;;
      *)
        if [[ -n "$SETUP_SQL_INPUT_PATH" ]]; then
          setup_sql::die "expected exactly one input YAML file"
        fi
        SETUP_SQL_INPUT_PATH="$1"
        shift
        ;;
    esac
  done

  [[ -n "$SETUP_SQL_INPUT_PATH" ]] || setup_sql::die "input YAML file is required"
  [[ -f "$SETUP_SQL_INPUT_PATH" ]] || setup_sql::die "input YAML file not found: $SETUP_SQL_INPUT_PATH"
}

setup_sql::emit_file() {
  local path="$1"
  local contents="$2"

  if [[ "$SETUP_SQL_DRY_RUN" == 'true' ]]; then
    printf '=== %s ===\n%s' "$path" "$contents"
    return
  fi

  mkdir -p "$(dirname "$path")"
  printf '%s' "$contents" >"$path"
}

setup_sql::yaml_backend() {
  if command -v yq >/dev/null 2>&1; then
    printf 'yq\n'
    return
  fi

  if command -v python3 >/dev/null 2>&1; then
    printf 'python3\n'
    return
  fi

  setup_sql::die "required command not found: yq or python3"
}

setup_sql::path_to_yq() {
  local path="$1"
  local expression=''
  local part
  local -a parts

  IFS='.' read -r -a parts <<<"$path"
  for part in "${parts[@]}"; do
    if [[ "$part" =~ ^[0-9]+$ ]]; then
      expression+="[$part]"
    else
      expression+=".$part"
    fi
  done

  printf '%s\n' "$expression"
}

setup_sql::python_yaml_query() {
  local config_path="$1"
  local query_path="$2"
  local query_mode="$3"

  python3 - "$config_path" "$query_path" "$query_mode" <<'PY'
import sys


def parse_scalar(text: str):
    if text in {"null", "~"}:
        return None
    if len(text) >= 2 and text[0] == text[-1] and text[0] in {"'", '"'}:
        return text[1:-1]
    return text


def preprocess(path: str):
    lines = []
    for raw_line in open(path, encoding="utf-8").read().splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(raw_line) - len(raw_line.lstrip(" "))
        lines.append((indent, stripped))
    return lines


def parse_block(lines, start, indent):
    if start >= len(lines):
        return None, start

    current_indent, current_text = lines[start]
    if current_indent < indent:
        return None, start

    if current_text.startswith("- "):
        items = []
        index = start
        while index < len(lines):
            line_indent, line_text = lines[index]
            if line_indent < indent or line_indent != indent or not line_text.startswith("- "):
                break

            item_text = line_text[2:].strip()
            if not item_text:
                child, index = parse_block(lines, index + 1, indent + 2)
                items.append(child)
                continue

            if ":" in item_text:
                key, value = item_text.split(":", 1)
                entry = {}
                key = key.strip()
                value = value.strip()
                if value:
                    entry[key] = parse_scalar(value)
                    index += 1
                else:
                    child, index = parse_block(lines, index + 1, indent + 2)
                    entry[key] = child

                while index < len(lines):
                    nested_indent, nested_text = lines[index]
                    if nested_indent < indent + 2 or nested_indent != indent + 2 or nested_text.startswith("- "):
                        break
                    nested_key, nested_value = nested_text.split(":", 1)
                    nested_key = nested_key.strip()
                    nested_value = nested_value.strip()
                    if nested_value:
                        entry[nested_key] = parse_scalar(nested_value)
                        index += 1
                    else:
                        child, index = parse_block(lines, index + 1, indent + 4)
                        entry[nested_key] = child

                items.append(entry)
                continue

            items.append(parse_scalar(item_text))
            index += 1

        return items, index

    mapping = {}
    index = start
    while index < len(lines):
        line_indent, line_text = lines[index]
        if line_indent < indent or line_indent != indent or line_text.startswith("- "):
            break

        key, value = line_text.split(":", 1)
        key = key.strip()
        value = value.strip()
        if value:
            mapping[key] = parse_scalar(value)
            index += 1
        else:
            child, index = parse_block(lines, index + 1, indent + 2)
            mapping[key] = child

    return mapping, index


def resolve_path(value, query_path: str):
    if not query_path:
        return value

    current = value
    for token in query_path.split("."):
        if token == "":
            continue
        if isinstance(current, list):
            current = current[int(token)]
        else:
            current = current[token]
    return current


lines = preprocess(sys.argv[1])
root, _ = parse_block(lines, 0, 0)
path = sys.argv[2]
mode = sys.argv[3]

try:
    value = resolve_path(root, path)
except (KeyError, IndexError, TypeError, ValueError):
    if mode == "length":
        print("0", end="")
    sys.exit(0)

if mode == "scalar":
    if value is None:
        print("", end="")
    elif isinstance(value, (dict, list)):
        raise SystemExit("scalar query resolved to non-scalar value")
    else:
        print(str(value), end="")
elif mode == "list":
    if value is None:
        sys.exit(0)
    if not isinstance(value, list):
        raise SystemExit("list query resolved to non-list value")
    for item in value:
        if isinstance(item, (dict, list)):
            raise SystemExit("list query resolved to non-scalar item")
        print("" if item is None else str(item))
elif mode == "length":
    if value is None:
        print("0", end="")
    elif isinstance(value, (dict, list, str)):
        print(str(len(value)), end="")
    else:
        print("0", end="")
else:
    raise SystemExit(f"unknown query mode: {mode}")
PY
}

setup_sql::yaml_scalar() {
  local config_path="$1"
  local query_path="$2"
  local backend

  backend="$(setup_sql::yaml_backend)"
  if [[ "$backend" == 'yq' ]]; then
    yq -r "$(setup_sql::path_to_yq "$query_path") // \"\"" "$config_path"
    return
  fi

  setup_sql::python_yaml_query "$config_path" "$query_path" scalar
}

setup_sql::yaml_list() {
  local config_path="$1"
  local query_path="$2"
  local backend

  backend="$(setup_sql::yaml_backend)"
  if [[ "$backend" == 'yq' ]]; then
    yq -r "$(setup_sql::path_to_yq "$query_path")[]?" "$config_path"
    return
  fi

  setup_sql::python_yaml_query "$config_path" "$query_path" list
}

setup_sql::yaml_length() {
  local config_path="$1"
  local query_path="$2"
  local backend

  backend="$(setup_sql::yaml_backend)"
  if [[ "$backend" == 'yq' ]]; then
    yq -r "$(setup_sql::path_to_yq "$query_path") | length" "$config_path"
    return
  fi

  setup_sql::python_yaml_query "$config_path" "$query_path" length
}
