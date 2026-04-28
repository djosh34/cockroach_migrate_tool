#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  image-artifacts.sh export-platform --git-sha <sha> --output-dir <dir>
  image-artifacts.sh assemble-multi-platform --git-sha <sha> --input-dir <dir> --output-dir <dir>
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_env() {
  local name
  for name in "$@"; do
    [[ -n "${!name:-}" ]] || fail "required environment variable \`${name}\` is not set"
  done
}

canonicalize_path() {
  realpath -m "$1"
}

require_git_sha() {
  [[ "$1" =~ ^[0-9a-f]{7,40}$ ]] || fail "git sha must be 7-40 lowercase hex characters, found \`$1\`"
}

json_file_digest() {
  sha256sum "$1" | awk '{ print $1 }'
}

json_file_size() {
  stat -c '%s' "$1"
}

copy_layout_blobs() {
  local source_layout=$1
  local destination_layout=$2

  mkdir -p "$destination_layout/blobs/sha256"
  find "$source_layout/blobs/sha256" -type f -print0 \
    | while IFS= read -r -d '' blob_path; do
        cp -n "$blob_path" "$destination_layout/blobs/sha256/"
      done
}

write_platform_metadata() {
  local metadata_path=$1
  local git_sha=$2
  local image_id=$3
  local image_name=$4
  local package_attr=$5
  local layout_rel_path=$6
  local source_archive_path=$7
  local manifest_media_type=$8
  local manifest_digest=$9
  local manifest_size=${10}

  jq -n \
    --arg git_sha "$git_sha" \
    --arg image_id "$image_id" \
    --arg image_name "$image_name" \
    --arg package_attr "$package_attr" \
    --arg layout_rel_path "$layout_rel_path" \
    --arg source_archive_path "$source_archive_path" \
    --arg platform "$CI_OCI_PLATFORM" \
    --arg os "$CI_OCI_OS" \
    --arg architecture "$CI_OCI_ARCHITECTURE" \
    --arg manifest_media_type "$manifest_media_type" \
    --arg manifest_digest "$manifest_digest" \
    --argjson manifest_size "$manifest_size" \
    '{
      git_sha: $git_sha,
      image_id: $image_id,
      image_name: $image_name,
      package_attr: $package_attr,
      oci_layout_rel_path: $layout_rel_path,
      source_archive_path: $source_archive_path,
      platform_id: ($os + "-" + $architecture),
      platform: {
        platform: $platform,
        os: $os,
        architecture: $architecture
      },
      manifest: {
        mediaType: $manifest_media_type,
        digest: $manifest_digest,
        size: $manifest_size
      }
    }' >"$metadata_path"
}

export_platform_artifacts() {
  require_env CI_IMAGE_SPECS_JSON CI_OCI_ARCHITECTURE CI_OCI_OS CI_OCI_PLATFORM

  local git_sha=""
  local output_dir=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --git-sha)
        git_sha=${2:-}
        shift 2
        ;;
      --output-dir)
        output_dir=${2:-}
        shift 2
        ;;
      *)
        usage
        fail "unknown export-platform argument \`$1\`"
        ;;
    esac
  done

  [[ -n "$git_sha" ]] || fail "export-platform requires --git-sha"
  [[ -n "$output_dir" ]] || fail "export-platform requires --output-dir"
  require_git_sha "$git_sha"

  output_dir=$(canonicalize_path "$output_dir")
  mkdir -p "$output_dir/images"

  while IFS= read -r image_spec; do
    local image_id image_name package_attr artifact_dir layout_dir metadata_path build_output archive_path manifest_descriptor manifest_digest manifest_media_type manifest_size
    image_id=$(jq -r '.image_id' <<<"$image_spec")
    image_name=$(jq -r '.image_name' <<<"$image_spec")
    package_attr=$(jq -r '.package_attr' <<<"$image_spec")
    artifact_dir="$output_dir/images/$image_id"
    layout_dir="$artifact_dir/layout"
    metadata_path="$artifact_dir/metadata.json"

    mkdir -p "$artifact_dir"

    build_output=$(nix build --no-link --print-out-paths ".#${package_attr}")
    archive_path=$(printf '%s' "$build_output" | tail -n 1)
    [[ -f "$archive_path" ]] || fail "nix build for \`${package_attr}\` did not produce a file output"

    rm -rf "$layout_dir"
    skopeo copy --insecure-policy \
      "docker-archive:${archive_path}" \
      "oci:${layout_dir}:${git_sha}"
    skopeo inspect --raw "oci:${layout_dir}:${git_sha}" >/dev/null

    manifest_descriptor=$(jq -cer --arg git_sha "$git_sha" '.manifests[] | select(.annotations["org.opencontainers.image.ref.name"] == $git_sha)' "$layout_dir/index.json")
    manifest_digest=$(jq -r '.digest' <<<"$manifest_descriptor")
    manifest_media_type=$(jq -r '.mediaType' <<<"$manifest_descriptor")
    manifest_size=$(jq -r '.size' <<<"$manifest_descriptor")

    write_platform_metadata \
      "$metadata_path" \
      "$git_sha" \
      "$image_id" \
      "$image_name" \
      "$package_attr" \
      "layout" \
      "$archive_path" \
      "$manifest_media_type" \
      "$manifest_digest" \
      "$manifest_size"
  done < <(jq -c '.[]' <<<"$CI_IMAGE_SPECS_JSON")
}

assemble_multi_platform_artifacts() {
  require_env CI_IMAGE_SPECS_JSON

  local git_sha=""
  local input_dir=""
  local output_dir=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --git-sha)
        git_sha=${2:-}
        shift 2
        ;;
      --input-dir)
        input_dir=${2:-}
        shift 2
        ;;
      --output-dir)
        output_dir=${2:-}
        shift 2
        ;;
      *)
        usage
        fail "unknown assemble-multi-platform argument \`$1\`"
        ;;
    esac
  done

  [[ -n "$git_sha" ]] || fail "assemble-multi-platform requires --git-sha"
  [[ -n "$input_dir" ]] || fail "assemble-multi-platform requires --input-dir"
  [[ -n "$output_dir" ]] || fail "assemble-multi-platform requires --output-dir"
  require_git_sha "$git_sha"

  input_dir=$(canonicalize_path "$input_dir")
  output_dir=$(canonicalize_path "$output_dir")
  [[ -d "$input_dir" ]] || fail "input directory \`$input_dir\` does not exist"
  mkdir -p "$output_dir"

  mapfile -t metadata_files < <(find "$input_dir" -type f -name metadata.json | sort)
  ((${#metadata_files[@]} > 0)) || fail "no platform artifact metadata files found under \`$input_dir\`"

  while IFS= read -r image_spec; do
    local image_id image_name final_layout descriptors_file platforms_file matched_metadata_files
    image_id=$(jq -r '.image_id' <<<"$image_spec")
    image_name=$(jq -r '.image_name' <<<"$image_spec")
    final_layout="$output_dir/$image_name"
    descriptors_file=$(mktemp)
    platforms_file=$(mktemp)
    matched_metadata_files=()

    for metadata_path in "${metadata_files[@]}"; do
      if jq -e --arg git_sha "$git_sha" --arg image_id "$image_id" '.git_sha == $git_sha and .image_id == $image_id' "$metadata_path" >/dev/null; then
        matched_metadata_files+=("$metadata_path")
      fi
    done

    ((${#matched_metadata_files[@]} >= 2)) || fail "expected at least two platform artifacts for \`${image_name}:${git_sha}\`, found ${#matched_metadata_files[@]}"

    rm -rf "$final_layout"
    mkdir -p "$final_layout/blobs/sha256"
    printf '{"imageLayoutVersion":"1.0.0"}\n' >"$final_layout/oci-layout"

    for metadata_path in "${matched_metadata_files[@]}"; do
      local source_layout platform_id os architecture manifest_media_type manifest_digest manifest_size
      source_layout=$(canonicalize_path "$(dirname "$metadata_path")/$(jq -r '.oci_layout_rel_path' "$metadata_path")")
      platform_id=$(jq -r '.platform_id' "$metadata_path")
      os=$(jq -r '.platform.os' "$metadata_path")
      architecture=$(jq -r '.platform.architecture' "$metadata_path")
      manifest_media_type=$(jq -r '.manifest.mediaType' "$metadata_path")
      manifest_digest=$(jq -r '.manifest.digest' "$metadata_path")
      manifest_size=$(jq -r '.manifest.size' "$metadata_path")

      printf '%s\n' "$platform_id" >>"$platforms_file"
      copy_layout_blobs "$source_layout" "$final_layout"
      jq -n \
        --arg mediaType "$manifest_media_type" \
        --arg digest "$manifest_digest" \
        --arg os "$os" \
        --arg architecture "$architecture" \
        --arg platform_id "$platform_id" \
        --argjson size "$manifest_size" \
        '{
          mediaType: $mediaType,
          digest: $digest,
          size: $size,
          platform: {
            os: $os,
            architecture: $architecture
          },
          annotations: {
            "io.cockroach-migrate.platform-id": $platform_id
          }
        }' >>"$descriptors_file"
    done

    sort -u "$platforms_file" | awk 'END { print NR }' | {
      read -r distinct_platform_count
      ((distinct_platform_count >= 2)) || fail "expected at least two distinct platforms for \`${image_name}:${git_sha}\`"
    }

    local multi_platform_manifest_source multi_platform_manifest_digest multi_platform_manifest_size top_level_index
    multi_platform_manifest_source=$(mktemp)
    jq -s '{
      schemaVersion: 2,
      mediaType: "application/vnd.oci.image.index.v1+json",
      manifests: .
    }' "$descriptors_file" >"$multi_platform_manifest_source"

    multi_platform_manifest_digest=$(json_file_digest "$multi_platform_manifest_source")
    multi_platform_manifest_size=$(json_file_size "$multi_platform_manifest_source")
    mv "$multi_platform_manifest_source" "$final_layout/blobs/sha256/$multi_platform_manifest_digest"

    top_level_index="$final_layout/index.json"
    jq -n \
      --arg git_sha "$git_sha" \
      --arg digest "sha256:${multi_platform_manifest_digest}" \
      --argjson size "$multi_platform_manifest_size" \
      '{
        schemaVersion: 2,
        mediaType: "application/vnd.oci.image.index.v1+json",
        manifests: [
          {
            mediaType: "application/vnd.oci.image.index.v1+json",
            digest: $digest,
            size: $size,
            annotations: {
              "org.opencontainers.image.ref.name": $git_sha
            }
          }
        ]
      }' >"$top_level_index"

    skopeo inspect --raw "oci:${final_layout}:${git_sha}" \
      | jq -e '.manifests | length >= 2' >/dev/null

    jq -n \
      --arg git_sha "$git_sha" \
      --arg image_name "$image_name" \
      --arg layout_path "$final_layout" \
      --arg push_example "crane push --index ${final_layout} ghcr.io/<owner>/${image_name}:${git_sha}" \
      --slurpfile platforms "$descriptors_file" \
      '{
        git_sha: $git_sha,
        image_name: $image_name,
        oci_layout_path: $layout_path,
        later_publish_example: $push_example,
        platforms: ($platforms | map(.platform))
      }' >"$final_layout/artifact-metadata.json"

    rm -f "$descriptors_file" "$platforms_file"
  done < <(jq -c '.[]' <<<"$CI_IMAGE_SPECS_JSON")
}

main() {
  [[ $# -ge 1 ]] || {
    usage
    exit 1
  }

  local subcommand=$1
  shift

  case "$subcommand" in
    export-platform)
      export_platform_artifacts "$@"
      ;;
    assemble-multi-platform)
      assemble_multi_platform_artifacts "$@"
      ;;
    *)
      usage
      fail "unknown subcommand \`$subcommand\`"
      ;;
  esac
}

main "$@"
