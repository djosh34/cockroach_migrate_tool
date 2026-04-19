#!/usr/bin/env bash
set -euxo pipefail
set -m

usage() {
 echo "Usage: $0 [OPTIONS]"
 echo "Options:"
 echo " -h, --help              Display this help message"
 echo " --cloud                 Specifies the cloud to run testing on (aws or gcp)"
 echo " --cluster-name          The name of the Roachprod cluster"
 echo " -z, --zones             Zone(s) to spin up the cluster (must match the cloud provider you're using); comma separated"
 echo " -n, --nodes             Number of nodes to spin up"
 echo " --volume-size           Size in GB of volume (EBS for AWS, PD for GCP)"
 echo " --machine-type          Machine type that you want to spin up (reference AWS and GCP docs)"
 echo " --aws-cpu-options       CPU option for AWS machine"
 echo " --aws-machine-type-ssd  Machine type that you want to spin up (reference AWS and GCP docs)"
 echo " --source                Source connection string"
 echo " --fetch-args            Arguments to run fetch with"
 echo " --verify-args           Arguments to run verify with"
 echo " --cluster-version       CRDB version for cluster. If empty, runs on the latest release branch."
}

# Dependency functions
has_argument() {
    [[ ("$1" == *=* && -n ${1#*=}) || ( ! -z "$2" && "$2" != -*)  ]];
}

extract_argument() {
  echo "${2:-${1#*=}}"
}

export ROACHPROD_USER=migrations
# Read flags
CLUSTER=""
CLUSTER_VERSION=""
PROVIDER="aws"
ZONE="us-east-1a"
NUM_NODES=4
VOLUME_SIZE=""
MACHINE_TYPE=""
AWS_CPU_OPTIONS=""
AWS_MACHINE_TYPE_SSD=""
SOURCE=""
FETCH_ARGS=""
VERIFY_ARGS=""

# TODO instance size and settings.
handle_options() {
  while [ $# -gt 0 ]; do
    case $1 in
      -h | --help)
        usage
        exit 0
        ;;
      --cloud)
        PROVIDER=$(extract_argument $@)
        shift
        ;;
      --cluster-name)
        if ! has_argument $@; then
          echo "Cluster name not specified." >&2
          usage
          exit 1
        fi
        CLUSTER=$(extract_argument $@)

        shift
        ;;
      --cluster-version)
        CLUSTER_VERSION=$(extract_argument $@)

        shift
        ;;
      -z | --zones)
        ZONE=$(extract_argument $@)

        shift
        ;;
      -n | --nodes)
        NUM_NODES=$(extract_argument $@)

        shift
        ;;
      --volume-size)
        VOLUME_SIZE=$(extract_argument $@)

        shift
        ;;
      --machine-type)
        MACHINE_TYPE=$(extract_argument $@)

        shift
        ;;
      # AWS specific flags
      --aws-cpu-options)
        AWS_CPU_OPTIONS=$(extract_argument $@)

        shift
        ;;
      --aws-machine-type-ssd)
        AWS_MACHINE_TYPE_SSD=$(extract_argument $@)

        shift
        ;;
      --source)
        SOURCE=$(extract_argument $@)

        shift
        ;;
      --fetch-args)
        FETCH_ARGS=$(extract_argument "$@")

        shift
        ;;
      --verify-args)
        VERIFY_ARGS=$(extract_argument "$@")

        shift
        ;;
      *)
        echo "Invalid option: $1" >&2
        usage
        exit 1
        ;;
    esac
    shift
  done
}

handle_options "$@"

# To create a unique name for cluster and version,
# we can concatenate the two.
CLEANED_CLUSTER_VERSION=$(echo "$CLUSTER_VERSION" | sed 's/\./-/g')
CLUSTER="$CLUSTER-$CLEANED_CLUSTER_VERSION"

export CLUSTER

echo "Listing out all parameters"
echo "Cloud Provider: $PROVIDER";
echo "Zone: $ZONE";
echo "Num Nodes: $NUM_NODES";
echo "Cluster: $CLUSTER";
echo "Cluster Version: $CLUSTER_VERSION";
echo "Volume Size: $VOLUME_SIZE";
echo "Machine Type: $MACHINE_TYPE";
echo "AWS CPU Options: $AWS_CPU_OPTIONS";
echo "AWS Machine Type SSD: $AWS_MACHINE_TYPE_SSD";
echo "Fetch Args: $FETCH_ARGS"
echo "Verify Args: $VERIFY_ARGS"

ROACHPROD_ARGS=(
    --clouds $PROVIDER
    -n $NUM_NODES
)
if [[ "$VOLUME_SIZE" != "" ]]; then 
    ROACHPROD_ARGS+=(--aws-ebs-volume-size $VOLUME_SIZE)
fi

echo ${ROACHPROD_ARGS[@]}

# We want to clean up the cluster and roachprod binary.
trap "./roachprod destroy $CLUSTER" EXIT

# Setup Roachprod
chmod +x roachprod

# Conditional logic for AWS vs. GCE.
if [[ "$PROVIDER" == "aws" ]]; then
    if [[ "$VOLUME_SIZE" != "" ]]; then 
        ROACHPROD_ARGS+=(--aws-ebs-volume-size $VOLUME_SIZE)
    fi

    if [[ "$MACHINE_TYPE" != "" ]]; then 
        ROACHPROD_ARGS+=(--aws-machine-type $MACHINE_TYPE)
    fi

    if [[ "$AWS_CPU_OPTIONS" != "" ]]; then 
        ROACHPROD_ARGS+=(--aws-cpu-options $AWS_CPU_OPTIONS)
    fi

    if [[ "$AWS_MACHINE_TYPE_SSD" != "" ]]; then 
        ROACHPROD_ARGS+=(--aws-machine-type-ssd $AWS_MACHINE_TYPE_SSD)
    fi

    ./roachprod create $CLUSTER --aws-zones $ZONE "${ROACHPROD_ARGS[@]}"
elif [[ "$PROVIDER" == "gcp" ]]; then
    if [[ "$VOLUME_SIZE" != "" ]]; then 
        ROACHPROD_ARGS+=(--gce-pd-volume-size $VOLUME_SIZE)
    fi

    if [[ "$MACHINE_TYPE" != "" ]]; then 
        ROACHPROD_ARGS+=(--gce-machine-type $MACHINE_TYPE)
    fi

    ./roachprod create $CLUSTER --gce-zones $ZONE "${ROACHPROD_ARGS[@]}"
fi

if [[ "$CLUSTER_VERSION" == "" || "$CLUSTER_VERSION" == "latest" ]]; then
  # In the case that no cluster version is set or we explicitly set latest, 
  # we want to test on the latest alpha branch.
  # Not specifying a version after "cockroach" will default pull the latest.
  ./roachprod stage $CLUSTER cockroach
else
  # In the case that we set a version, we want to test on that version.
  ./roachprod stage $CLUSTER release "v$CLUSTER_VERSION" --os linux
fi
./roachprod start $CLUSTER

# Put binary on
./roachprod put $CLUSTER molt

/usr/local/go/bin/go test -v -timeout 10000s -run ^TestScale$ github.com/cockroachdb/molt/scaletesting --roachprod-cmd "$PWD/roachprod" --cluster "$CLUSTER" --source "$SOURCE" --fetch-args "$FETCH_ARGS" --verify-args "$VERIFY_ARGS"

exit 0