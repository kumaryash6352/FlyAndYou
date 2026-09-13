#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'The Rust/Metal application requires macOS.' >&2
  exit 1
fi
if [ ! -x .venv/bin/python ] || [ ! -f data/cache/malecns-v1/manifest.json ]; then
  echo 'Run ./scripts/setup.sh once to prepare the local controller.' >&2
  exit 1
fi
export FLY_AND_YOU_ROOT="$PWD"
.venv/bin/python -m controller.brainworker.export_rust
cargo build --release --locked -p fly-brain-worker
target/release/fly-brain-worker --prepare-tutorial
cargo build --workspace --locked
exec target/debug/fly-and-you "$@"
