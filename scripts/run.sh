#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ ! -x .venv/bin/python ] || [ ! -f data/cache/malecns-v1/manifest.json ]; then
  echo 'Run ./scripts/setup.sh once to prepare the local controller.' >&2
  exit 1
fi
export FLY_AND_YOU_ROOT="$PWD"
.venv/bin/python -m controller.brainworker.tutorial
exec cargo run --locked -p fly-and-you -- "$@"
