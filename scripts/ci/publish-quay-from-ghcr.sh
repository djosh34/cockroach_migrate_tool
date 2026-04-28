#!/usr/bin/env bash

set -euo pipefail

required_env_vars=(
  GHCR_PUBLISH_SUMMARY_PATH
  GHCR_USERNAME
  GHCR_PASSWORD
  QUAY_ORGANIZATION
  RUNNER_IMAGE_REPOSITORY
  VERIFY_IMAGE_REPOSITORY
  QUAY_USERNAME
  QUAY_PASSWORD
)

for required_env_var in "${required_env_vars[@]}"; do
  if [[ -z "${!required_env_var:-}" ]]; then
    echo "error: ${required_env_var} is required" >&2
    exit 1
  fi
done

required_commands=(curl jq skopeo)
for required_command in "${required_commands[@]}"; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "error: missing required command: ${required_command}" >&2
    exit 1
  fi
done

ghcr_publish_summary_path="${GHCR_PUBLISH_SUMMARY_PATH}"
quay_publish_summary_path="${QUAY_PUBLISH_SUMMARY_PATH:-}"
quay_organization="${QUAY_ORGANIZATION}"
runner_image_repository="${RUNNER_IMAGE_REPOSITORY}"
verify_image_repository="${VERIFY_IMAGE_REPOSITORY}"

if [[ ! -s "$ghcr_publish_summary_path" ]]; then
  echo "error: expected non-empty GHCR publish summary at ${ghcr_publish_summary_path}" >&2
  exit 1
fi

declare -A destination_repositories=(
  ["runner-image"]="${runner_image_repository}"
  ["verify-image"]="${verify_image_repository}"
)

declare -A quay_refs
declare -A quay_digests
declare -A quay_platforms
declare -A quay_security_statuses
declare -A quay_security_counts

summary_image_string_field() {
  local logical_name="$1"
  local field_name="$2"

  jq -er \
    --arg logical_name "$logical_name" \
    --arg field_name "$field_name" \
    '.images[]
      | select(.logical_name == $logical_name)
      | .[$field_name]
      | strings' \
    "$ghcr_publish_summary_path"
}

summary_image_json_field() {
  local logical_name="$1"
  local field_name="$2"

  jq -ec \
    --arg logical_name "$logical_name" \
    --arg field_name "$field_name" \
    '.images[]
      | select(.logical_name == $logical_name)
      | .[$field_name]' \
    "$ghcr_publish_summary_path"
}

security_counts_json() {
  local security_response="$1"

  jq -c '
    [
      .. | objects
      | (.Severity? // .severity? // .priority? // .Priority?)
      | select(type == "string" and length > 0)
      | ascii_upcase
    ]
    | reduce .[] as $severity (
        {
          UNKNOWN: 0,
          LOW: 0,
          MEDIUM: 0,
          HIGH: 0,
          CRITICAL: 0
        };
        if has($severity) then
          .[$severity] += 1
        else
          .UNKNOWN += 1
        end
      )
  ' <<<"$security_response"
}

copy_and_report_image() {
  local logical_name="$1"
  local source_ref source_digest source_platforms destination_repository destination_ref
  local source_inspect_json destination_inspect_json destination_raw_manifest destination_digest destination_platforms_json
  local security_url security_response security_status security_counts

  source_ref="$(summary_image_string_field "$logical_name" final_ref)"
  source_digest="$(summary_image_string_field "$logical_name" digest)"
  source_platforms="$(summary_image_json_field "$logical_name" platforms)"
  destination_repository="${destination_repositories["$logical_name"]:-}"

  if [[ -z "$destination_repository" ]]; then
    echo "error: missing destination repository mapping for ${logical_name}" >&2
    exit 1
  fi

  destination_ref="quay.io/${quay_organization}/${destination_repository}:$(jq -r '.git_sha' "$ghcr_publish_summary_path")"

  echo "Copy tool: $(command -v skopeo)"
  echo "Copying multi-platform image without rebuild:"
  echo "  source_ref=${source_ref}"
  echo "  source_digest=${source_digest}"
  echo "  source_platforms=${source_platforms}"
  echo "  destination_ref=${destination_ref}"

  source_inspect_json="$(
    skopeo inspect \
      --creds "${GHCR_USERNAME}:${GHCR_PASSWORD}" \
      "docker://${source_ref}"
  )"

  if [[ "$(jq -r '.Digest // empty' <<<"$source_inspect_json")" != "$source_digest" ]]; then
    echo "error: GHCR publish summary digest does not match current source digest for ${source_ref}" >&2
    exit 1
  fi

  skopeo copy \
    --all \
    --src-creds "${GHCR_USERNAME}:${GHCR_PASSWORD}" \
    --dest-creds "${QUAY_USERNAME}:${QUAY_PASSWORD}" \
    "docker://${source_ref}" \
    "docker://${destination_ref}"

  destination_inspect_json="$(
    skopeo inspect \
      --creds "${QUAY_USERNAME}:${QUAY_PASSWORD}" \
      "docker://${destination_ref}"
  )"
  destination_raw_manifest="$(
    skopeo inspect \
      --raw \
      --creds "${QUAY_USERNAME}:${QUAY_PASSWORD}" \
      "docker://${destination_ref}"
  )"

  destination_digest="$(jq -r '.Digest // empty' <<<"$destination_inspect_json")"
  if [[ -z "$destination_digest" ]]; then
    echo "error: skopeo inspect did not return a digest for ${destination_ref}" >&2
    exit 1
  fi

  destination_platforms_json="$(
    jq -c '
      if (.manifests // null) == null then
        []
      else
        [
          .manifests[]
          | .platform
          | [ .os, .architecture, .variant ]
          | map(select(. != null and . != ""))
          | join("/")
        ]
      end
    ' <<<"$destination_raw_manifest"
  )"

  echo "Published Quay image:"
  echo "  destination_ref=${destination_ref}"
  echo "  destination_digest=${destination_digest}"
  echo "  destination_platforms=${destination_platforms_json}"

  security_url="https://quay.io/api/v1/repository/${quay_organization}/${destination_repository}/manifest/${destination_digest}/security?vulnerabilities=true"
  echo "Security API query:"
  echo "  url=${security_url}"
  echo "  tool=$(command -v curl)"
  echo "  policy=report-only for discovered vulnerabilities; fail on copy/inspect/API ambiguity"

  security_response="$(
    curl \
      --fail-with-body \
      --silent \
      --show-error \
      -H "Accept: application/json" \
      "$security_url"
  )"

  echo "Security API response:"
  echo "$security_response" | jq .

  security_status="$(jq -r '.status // empty' <<<"$security_response")"
  if [[ -z "$security_status" ]]; then
    echo "error: Quay security response for ${destination_ref} did not include a status" >&2
    exit 1
  fi

  if [[ "$(jq -r '.data != null' <<<"$security_response")" == "true" ]]; then
    security_counts="$(security_counts_json "$security_response")"
    echo "Quay vulnerability severity counts for ${destination_ref}:"
    echo "${security_counts}" | jq .
  else
    case "$security_status" in
      queued|scanning|pending|unsupported|unavailable|not_yet_scanned)
        security_counts='null'
        echo "Quay scanner state for ${destination_ref}: ${security_status}"
        ;;
      *)
        echo "error: unexpected Quay security status for ${destination_ref}: ${security_status}" >&2
        exit 1
        ;;
    esac
  fi

  quay_refs["$logical_name"]="$destination_ref"
  quay_digests["$logical_name"]="$destination_digest"
  quay_platforms["$logical_name"]="$destination_platforms_json"
  quay_security_statuses["$logical_name"]="$security_status"
  quay_security_counts["$logical_name"]="$security_counts"
}

write_quay_publish_summary() {
  if [[ -z "$quay_publish_summary_path" ]]; then
    return
  fi

  mkdir -p "$(dirname "$quay_publish_summary_path")"

  jq -n \
    --arg quay_organization "$quay_organization" \
    --arg runner_ref "${quay_refs["runner-image"]}" \
    --arg runner_digest "${quay_digests["runner-image"]}" \
    --argjson runner_platforms "${quay_platforms["runner-image"]}" \
    --arg runner_security_status "${quay_security_statuses["runner-image"]}" \
    --argjson runner_security_counts "${quay_security_counts["runner-image"]}" \
    --arg verify_ref "${quay_refs["verify-image"]}" \
    --arg verify_digest "${quay_digests["verify-image"]}" \
    --argjson verify_platforms "${quay_platforms["verify-image"]}" \
    --arg verify_security_status "${quay_security_statuses["verify-image"]}" \
    --argjson verify_security_counts "${quay_security_counts["verify-image"]}" \
    '{
      quay_organization: $quay_organization,
      vulnerability_policy: "report-only",
      images: [
        {
          logical_name: "runner-image",
          final_ref: $runner_ref,
          digest: $runner_digest,
          platforms: $runner_platforms,
          security_status: $runner_security_status,
          severity_counts: $runner_security_counts
        },
        {
          logical_name: "verify-image",
          final_ref: $verify_ref,
          digest: $verify_digest,
          platforms: $verify_platforms,
          security_status: $verify_security_status,
          severity_counts: $verify_security_counts
        }
      ]
    }' >"$quay_publish_summary_path"

  echo "Wrote Quay publish summary to ${quay_publish_summary_path}"
  jq . "$quay_publish_summary_path"
}

copy_and_report_image runner-image
copy_and_report_image verify-image
write_quay_publish_summary
