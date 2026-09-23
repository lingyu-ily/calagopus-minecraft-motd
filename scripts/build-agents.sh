#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dist_dir="${project_root}/dist/agent"
mkdir -p "${dist_dir}"

rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
if ! command -v cargo-zigbuild >/dev/null 2>&1; then
  cargo install cargo-zigbuild --locked
fi

cd "${project_root}/agent"
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl

install -m 0755 target/x86_64-unknown-linux-musl/release/calagopus-minecraft-motd-agent \
  "${dist_dir}/calagopus-minecraft-motd-agent-x86_64"
install -m 0755 target/aarch64-unknown-linux-musl/release/calagopus-minecraft-motd-agent \
  "${dist_dir}/calagopus-minecraft-motd-agent-aarch64"

sha256sum "${dist_dir}"/calagopus-minecraft-motd-agent-*

