#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'The Rust/Metal application requires macOS.' >&2
  exit 1
fi
if [ -f data/cache/malecns-v1/manifest.json ]; then
  .venv/bin/python -m controller.brainworker.export_rust
fi
cargo build --release --locked -p fly-brain-worker
target/release/fly-brain-worker --prepare-tutorial
cargo build --workspace --locked
mkdir -p FlyAndYou.app/Contents/MacOS
cp packaging/Info.plist FlyAndYou.app/Contents/Info.plist
cp target/debug/fly-and-you FlyAndYou.app/Contents/MacOS/fly-and-you.next
mv FlyAndYou.app/Contents/MacOS/fly-and-you.next FlyAndYou.app/Contents/MacOS/fly-and-you

cp target/release/fly-brain-worker FlyAndYou.app/Contents/MacOS/fly-brain-worker.next
mv FlyAndYou.app/Contents/MacOS/fly-brain-worker.next FlyAndYou.app/Contents/MacOS/fly-brain-worker
mkdir -p FlyAndYou.app/Contents/Resources
cp THIRD_PARTY_NOTICES.md FlyAndYou.app/Contents/Resources/THIRD_PARTY_NOTICES.md
mkdir -p FlyAndYou.app/Contents/Resources/third_party
cp third_party/CANDLE-LICENSE-APACHE FlyAndYou.app/Contents/Resources/third_party/CANDLE-LICENSE-APACHE
