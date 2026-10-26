#!/usr/bin/env bash
# Build the sampler and assemble the installable package under dist/.
# Install: Hearthloom dashboard → ☰ → Integrations → embers →
# "install one you built yourself" → the path to this dist/ folder (or
# `hearthloom ember pack dist` and upload the zip).
#
# Apple Silicon only: the sidecar is built for aarch64-apple-darwin.
set -euo pipefail
cd "$(dirname "$0")"

# --locked builds exactly the dependencies in Cargo.lock. The path remap
# keeps home-directory paths (the cargo registry lives under $HOME) out of
# the binary's panic messages.
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=$HOME=~" \
  cargo build --release --locked --target aarch64-apple-darwin

rm -rf dist
mkdir -p dist/bin
cp hearthember.json dist/
cp target/aarch64-apple-darwin/release/hpresence dist/bin/
cp -R ui dist/ui

echo "package ready: $(pwd)/dist"
