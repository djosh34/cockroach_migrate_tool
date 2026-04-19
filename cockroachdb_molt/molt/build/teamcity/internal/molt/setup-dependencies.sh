#!/usr/bin/env bash
set -euxo pipefail
set -m

# Sets up all dependencies, authentication details, SSH, binaries.
setup() {
    echo "$GOOGLE_EPHEMERAL_CREDENTIALS" > creds.json
    gcloud auth activate-service-account --key-file=creds.json

    aws configure set aws_access_key_id %env.AWS_ACCESS_KEY_ID%;
    aws configure set aws_secret_access_key %env.AWS_SECRET_ACCESS_KEY%;
    aws configure set default.region "US-EAST-1";
    mkdir -p "$HOME/.ssh/"
    # Attempt to remove id_rsa if it already exists
    rm -rf "$HOME/.ssh/id_rsa" || true

    ssh-keygen -t rsa -q -f "$HOME/.ssh/id_rsa" -N ""
    ls ~/.ssh/
    # Download Roachprod Binary
    gcloud storage cp gs://migrations-fetch-ci-test/roachprod-binary/roachprod roachprod

    # Setup Golang.
    trap 'rm -f /tmp/go.tgz' EXIT
    curl -fsSL https://dl.google.com/go/go1.22.1.linux-amd64.tar.gz > /tmp/go.tgz
    sha256sum -c - <<EOF
aab8e15785c997ae20f9c88422ee35d962c4562212bb0f879d052a35c8307c7f  /tmp/go.tgz
EOF
    tar -C /usr/local -zxf /tmp/go.tgz && rm /tmp/go.tgz

    # Verify that go was properly installed.
    /usr/local/go/bin/go version

    # Build the binary for molt for linux/amd64, because that is what the Roachprod instance is running.
    GOARCH=amd64 GOOS=linux /usr/local/go/bin/go build -o ./molt .
}

setup