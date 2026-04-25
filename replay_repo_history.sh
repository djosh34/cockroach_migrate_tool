#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./replay_repo_history.sh [options]

Replay this repository's tracked file tree from the first commit to HEAD.

Options:
  --maxdepth N          Only print paths up to N path components. Default: 4.
  --ignore-dir NAME    Hide any path that passes through NAME. Repeatable.
                        Default: .ralph
  --delay SECONDS      Sleep between commits. Default: 0.15.
  --ref REF            Replay commits reachable from REF. Default: HEAD.
  --header             Show commit progress, hash, and subject above the tree.
  --no-header          Hide the commit header. Default.
  --no-clear           Do not reset the cursor before each commit.
  -h, --help           Show this help.

The script uses a temporary Git index:
  GIT_INDEX_FILE=<tmp> git read-tree <commit>
  GIT_INDEX_FILE=<tmp> git ls-files

It does not check out commits and does not touch the working tree.
EOF
}

maxdepth=4
delay=0.15
ref=HEAD
clear_screen=1
show_header=0
ignore_dirs=(".ralph")

while [[ $# -gt 0 ]]; do
  case "$1" in
    --maxdepth)
      if [[ $# -lt 2 ]]; then
        echo "error: --maxdepth requires a value" >&2
        exit 2
      fi
      maxdepth="$2"
      shift 2
      ;;
    --ignore-dir)
      if [[ $# -lt 2 ]]; then
        echo "error: --ignore-dir requires a value" >&2
        exit 2
      fi
      ignore_dirs+=("$2")
      shift 2
      ;;
    --delay)
      if [[ $# -lt 2 ]]; then
        echo "error: --delay requires a value" >&2
        exit 2
      fi
      delay="$2"
      shift 2
      ;;
    --ref)
      if [[ $# -lt 2 ]]; then
        echo "error: --ref requires a value" >&2
        exit 2
      fi
      ref="$2"
      shift 2
      ;;
    --header)
      show_header=1
      shift
      ;;
    --no-header)
      show_header=0
      shift
      ;;
    --no-clear)
      clear_screen=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! [[ "$maxdepth" =~ ^[0-9]+$ ]] || [[ "$maxdepth" -lt 1 ]]; then
  echo "error: --maxdepth must be a positive integer" >&2
  exit 2
fi

if ! [[ "$delay" =~ ^([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]]; then
  echo "error: --delay must be a non-negative number" >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

index_file="$(mktemp "${TMPDIR:-/tmp}/repo-history-index.XXXXXX")"
commits_file="$(mktemp "${TMPDIR:-/tmp}/repo-history-commits.XXXXXX")"
paths_file="$(mktemp "${TMPDIR:-/tmp}/repo-history-paths.XXXXXX")"

cleanup() {
  rm -f "$index_file" "$commits_file" "$paths_file"
}
trap cleanup EXIT

git rev-list --reverse "$ref" >"$commits_file"
total_commits="$(wc -l <"$commits_file" | tr -d ' ')"

if [[ "$total_commits" -eq 0 ]]; then
  echo "error: no commits found for ref: $ref" >&2
  exit 1
fi

render_paths() {
  awk -v maxdepth="$maxdepth" '
    BEGIN {
      for (i = 1; i < ARGC; i++) {
        ignore[ARGV[i]] = 1
      }
      ARGC = 1
    }
    {
      component_count = split($0, components, "/")

      for (i = 1; i <= component_count; i++) {
        if (components[i] in ignore) {
          next
        }
      }

      if (component_count > maxdepth) {
        next
      }

      for (i = 1; i < component_count; i++) {
        dir = components[1]
        for (j = 2; j <= i; j++) {
          dir = dir "/" components[j]
        }
        seen[dir "/"] = 1
      }
      seen[$0] = 1
    }
    END {
      for (path in seen) {
        print path
      }
    }
  ' "${ignore_dirs[@]}" <"$paths_file" | sort
}

commit_number=0
while IFS= read -r commit; do
  commit_number=$((commit_number + 1))

  GIT_INDEX_FILE="$index_file" git read-tree "$commit"
  GIT_INDEX_FILE="$index_file" git ls-files >"$paths_file"

  if [[ "$clear_screen" -eq 1 ]]; then
    printf '\033[H\033[J'
  fi

  if [[ "$show_header" -eq 1 ]]; then
    git show -s --format='%h %s' "$commit" |
      awk -v current="$commit_number" -v total="$total_commits" '{ print current "/" total " " $0 "\n" }'
  fi

  render_paths
  sleep "$delay"
done <"$commits_file"
