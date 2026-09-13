#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'The Rust/Metal application requires macOS.' >&2
  exit 1
fi
if [ ! -f data/cache/malecns-rust-v1/manifest.json ] || [ ! -f data/cache/tutorial-rust.json ]; then
  echo 'Run ./scripts/setup.sh once to prepare the embedded controller.' >&2
  exit 1
fi
export FLY_AND_YOU_ROOT="$PWD"
cargo build --release --locked -p fly-brain-worker
target/release/fly-brain-worker --prepare-tutorial
cargo build --release --locked -p fly-and-you --bin fly-and-you
exec target/release/fly-and-you "$@"
