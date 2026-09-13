#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'The app bundle is for macOS. Use scripts/run.sh on other platforms.' >&2
  exit 1
fi
if [ -f data/cache/malecns-v1/manifest.json ]; then
  .venv/bin/python -m controller.brainworker.tutorial
fi
cargo build --locked -p fly-and-you
mkdir -p FlyAndYou.app/Contents/MacOS
cp packaging/Info.plist FlyAndYou.app/Contents/Info.plist
cp target/debug/fly-and-you FlyAndYou.app/Contents/MacOS/fly-and-you.next
mv FlyAndYou.app/Contents/MacOS/fly-and-you.next FlyAndYou.app/Contents/MacOS/fly-and-you
