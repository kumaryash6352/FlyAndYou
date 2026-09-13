#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export FLY_AND_YOU_ROOT="$PWD"
if [ ! -f data/cache/malecns-rust-v1/manifest.json ]; then
  echo 'Run ./scripts/setup.sh once to prepare the controller.' >&2
  exit 1
fi
cargo build --release --locked -p fly-brain-worker
target/release/fly-brain-worker --prepare-tutorial
cargo build --locked -p fly-and-you --bin fly-trailer
exec target/debug/fly-trailer "$@"
