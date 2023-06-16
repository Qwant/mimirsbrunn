#! /bin/bash
set -euo pipefail
type -p shopt && shopt -s expand_aliases

source ci/bootstrap/src/k8s/bootstrap.sh

export PATH="$PATH:$PWD/ci/bootstrap/bin"
export no_proxy="$no_proxy,elasticsearch"
export NO_PROXY="$NO_PROXY,elasticsearch"
