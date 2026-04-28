#!/usr/bin/env bash

set -euo pipefail

if [[ -z "${ARTIFACT_ROOT:-}" ]]; then
  echo "error: ARTIFACT_ROOT is required" >&2
  exit 1
fi

if [[ -z "${GHCR_OWNER:-}" ]]; then
  echo "error: GHCR_OWNER is required" >&2
  exit 1
fi

if [[ -z "${GIT_SHA:-}" ]]; then
  echo "error: GIT_SHA is required" >&2
  exit 1
fi

if [[ -z "${RUN_ID:-}" ]]; then
  echo "error: RUN_ID is required" >&2
  exit 1
fi

dry_run="${DRY_RUN:-0}"

required_commands=(sed)
if [[ "$dry_run" != "1" ]]; then
  required_commands+=(docker)
fi

for required_command in "${required_commands[@]}"; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "error: missing required command: $required_command" >&2
    exit 1
  fi
done

artifact_root="${ARTIFACT_ROOT}"
ghcr_owner="${GHCR_OWNER,,}"
git_sha="${GIT_SHA}"
run_id="${RUN_ID}"
registry_prefix="ghcr.io/${ghcr_owner}"

declare -A temp_refs

read_metadata_value() {
  local metadata_file="$1"
  local key="$2"

  local value
  value="$(sed -n "s/^${key}=//p" "$metadata_file" | tail -n 1)"
  if [[ -z "$value" ]]; then
    echo "error: missing ${key} in ${metadata_file}" >&2
    exit 1
  fi

  printf '%s\n' "$value"
}

load_and_push_archive() {
  local image_name="$1"
  local arch="$2"
  local archive_path="$3"

  local temporary_ref load_output loaded_ref

  temporary_ref="${registry_prefix}/${image_name}:tmp-${git_sha}-${run_id}-${arch}"

  if [[ "$dry_run" == "1" ]]; then
    echo "Dry run: would load ${archive_path}"
    echo "Dry run: would push ${temporary_ref}"
    temp_refs["${image_name}:${arch}"]="$temporary_ref"
    return
  fi

  echo "Loading archive ${archive_path}"
  load_output="$(docker image load --input "$archive_path" 2>&1)"
  printf '%s\n' "$load_output"

  loaded_ref="$(printf '%s\n' "$load_output" | sed -n 's/^Loaded image: //p' | tail -n 1)"
  if [[ -z "$loaded_ref" ]]; then
    echo "error: docker image load did not report a loaded image reference for ${archive_path}" >&2
    exit 1
  fi

  echo "Tagging ${loaded_ref} as ${temporary_ref}"
  docker image tag "$loaded_ref" "$temporary_ref"

  echo "Pushing ${temporary_ref}"
  docker image push "$temporary_ref"

  temp_refs["${image_name}:${arch}"]="$temporary_ref"
}

publish_manifest() {
  local image_name="$1"
  local amd64_ref="$2"
  local arm64_ref="$3"
  local final_ref

  final_ref="${registry_prefix}/${image_name}:${git_sha}"

  if [[ "$dry_run" == "1" ]]; then
    echo "Dry run: would create ${final_ref} from ${amd64_ref} and ${arm64_ref}"
    return
  fi

  echo "Creating ${final_ref} from:"
  echo "  - ${amd64_ref}"
  echo "  - ${arm64_ref}"
  docker buildx imagetools create \
    --tag "$final_ref" \
    "$amd64_ref" \
    "$arm64_ref"

  echo "Inspecting ${final_ref}"
  docker buildx imagetools inspect "$final_ref"
}

shopt -s nullglob
metadata_files=("${artifact_root}"/*/metadata.env)
shopt -u nullglob

if [[ "${#metadata_files[@]}" -ne 4 ]]; then
  echo "error: expected 4 metadata files under ${artifact_root}, found ${#metadata_files[@]}" >&2
  exit 1
fi

for metadata_file in "${metadata_files[@]}"; do
  artifact_dir="$(dirname "$metadata_file")"
  image_name="$(read_metadata_value "$metadata_file" image)"
  arch="$(read_metadata_value "$metadata_file" arch)"
  archive_name="$(read_metadata_value "$metadata_file" archive_name)"
  archive_path="${artifact_dir}/${archive_name}"

  if [[ ! -s "$archive_path" ]]; then
    echo "error: expected non-empty archive ${archive_path}" >&2
    exit 1
  fi

  echo "Discovered artifact:"
  echo "  image=${image_name}"
  echo "  arch=${arch}"
  echo "  archive=${archive_path}"

  load_and_push_archive "$image_name" "$arch" "$archive_path"
done

for image_name in runner-image verify-image; do
  amd64_ref="${temp_refs["${image_name}:amd64"]:-}"
  arm64_ref="${temp_refs["${image_name}:arm64"]:-}"

  if [[ -z "$amd64_ref" || -z "$arm64_ref" ]]; then
    echo "error: missing temporary refs for ${image_name}" >&2
    exit 1
  fi

  publish_manifest "$image_name" "$amd64_ref" "$arm64_ref"
done
