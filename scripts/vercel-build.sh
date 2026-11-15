#!/usr/bin/env bash
# Builds the web version into dist/ on Vercel's build machine (Amazon Linux 2023, x86_64).
# Vercel runs this as the build command (see vercel.json). The machine has a C compiler, curl,
# tar and gzip, but no Rust and no Trunk, so both are fetched here.
#
# To try it locally in the same OS, see "Deploy to Vercel" in README.md.
set -euo pipefail

TRUNK_VERSION="0.22.1"
TOOLS="$HOME/.starwarden-tools"

cd "$(dirname "$0")/.."

# Rust with the WebAssembly target.
export PATH="$HOME/.cargo/bin:$TOOLS:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
    echo "==> Installing Rust"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain stable
fi
rustup target add wasm32-unknown-unknown

# Trunk, as a prebuilt static binary (compiling it would add several minutes).
if ! command -v trunk >/dev/null 2>&1; then
    echo "==> Installing Trunk $TRUNK_VERSION"
    mkdir -p "$TOOLS"
    curl -sSfL "https://github.com/trunk-rs/trunk/releases/download/v$TRUNK_VERSION/trunk-x86_64-unknown-linux-musl.tar.gz" |
        tar -xz -C "$TOOLS"
fi

rustc --version
trunk --version

echo "==> Building"
trunk build --release

echo "==> Done"
ls -l dist
