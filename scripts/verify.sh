#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test --workspace --locked
.venv/bin/python -m unittest discover -s reference -v
.venv/bin/python -m unittest discover -s controller/tests -v
if [ "${1:-}" = "--full" ]; then
  FLY_FULL_TEST=1 .venv/bin/python -m unittest discover -s controller/tests -p test_transport.py -v
fi
