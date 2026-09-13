#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test --workspace --locked
.venv/bin/python -m unittest discover -s reference -v
.venv/bin/python -m unittest discover -s controller/tests -v
if [ "${1:-}" = "--full" ]; then
  .venv/bin/python -m controller.brainworker.export_rust
  cargo build --release --locked -p fly-brain-worker
  FLY_RUST_TEST=1 .venv/bin/python -m unittest discover -s controller/tests -p test_rust_worker.py -v
  FLY_FULL_TEST=1 .venv/bin/python -m unittest discover -s controller/tests -p test_transport.py -v
fi
