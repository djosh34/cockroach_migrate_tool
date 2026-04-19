#!/usr/bin/env bash
set -euxo pipefail
set -m

VARIANT="${1:-base_aws}"
CLUSTER_VERSION="${2:-latest}"

# Get this current directory.
DIR="$(cd "$(dirname "$0")" && pwd)"

# Add new case statements for new variants
case $VARIANT in
  base_aws)
    $DIR/setup-scale-test.sh --cluster-name migrations-nightly-base-aws --cluster-version "$CLUSTER_VERSION" --source "$SOURCE_CONN" -n 4 --cloud aws --zones us-east-1a --machine-type m6idn.2xlarge --aws-machine-type-ssd m6idn.2xlarge \
    --fetch-args "--non-interactive --bucket-path 's3://migrations-fetch-ci-test/scale-test' --table-handling 'drop-on-target-and-recreate' --table-filter 'test_table_large' --export-concurrency 4" \
    --verify-args "--table-splits 2"
    ;;

  scaled_aws)
    $DIR/setup-scale-test.sh --cluster-name migrations-nightly-scaled-aws --cluster-version "$CLUSTER_VERSION" --source "$SOURCE_CONN" -n 4 --cloud aws --volume-size 1500 --zones us-east-1a --aws-cpu-options 'CoreCount=8,ThreadsPerCore=2' --machine-type m6idn.4xlarge --aws-machine-type-ssd m6idn.4xlarge \
    --fetch-args "--non-interactive --bucket-path 's3://migrations-fetch-ci-test/scale-test' --table-handling 'drop-on-target-and-recreate' --table-filter 'test_table_large' --export-concurrency 8" \
    --verify-args "--table-splits 4"
    ;;

  *)
    echo -n "unknown test variant"
    exit 1
    ;;
esac

exit 0