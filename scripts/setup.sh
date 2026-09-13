#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
uv sync --locked --python 3.12
mkdir -p data/raw
base='https://storage.googleapis.com/flyem-male-cns/v1.0/connectome-data/flat-connectome'
fetch() {
  if [ ! -f "data/raw/$1" ]; then
    curl -fL --retry 3 "$base/$2" -o "data/raw/$1.partial"
    mv "data/raw/$1.partial" "data/raw/$1"
  fi
}
fetch annotations.feather body-annotations-male-cns-v1.0-minconf-0.5.feather
fetch neurotransmitters.feather body-neurotransmitters-male-cns-v1.0.feather
fetch connections.feather connectome-weights-male-cns-v1.0-minconf-0.5.feather
if [ ! -f assets/brain_atlas.json ]; then
  .venv/bin/python -m controller.brainworker.anatomy
fi
.venv/bin/python -m controller.brainworker.prepare
.venv/bin/python -m controller.brainworker.tutorial
cargo build --locked -p fly-and-you
