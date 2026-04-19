#!/usr/bin/env bash
set -euxo pipefail
set -m

# Clean up the artifacts location.
rm -f /tmp/artifacts/molt*.log

./build/teamcity/internal/molt/setup-dependencies.sh

# Base test
./build/teamcity/internal/molt/test-variants.sh

supported_versions=("latest" "23.2.1" "23.1.12" "23.1.1" "22.2.1")
# Test supported versions (bigger machines for quickest)

for ver in "${supported_versions[@]}"; do
    ./build/teamcity/internal/molt/test-variants.sh scaled_aws "$ver"
done