#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Run this installer as root." >&2
  exit 1
fi

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "Usage: $0 <agent-binary> <panel-url> <enrollment-token> [listen-port]" >&2
  exit 1
fi

binary_path=$(readlink -f "$1")
panel_url=$2
enrollment_token=$3
listen_port=${4:-4001}
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

if [[ ! -f ${binary_path} ]]; then
  echo "Agent binary not found: ${binary_path}" >&2
  exit 1
fi
if [[ ! ${listen_port} =~ ^[0-9]+$ ]] || (( listen_port < 1 || listen_port > 65535 )); then
  echo "listen-port must be between 1 and 65535." >&2
  exit 1
fi

install -d -m 0700 /etc/calagopus-motd-agent
install -m 0755 "${binary_path}" /usr/local/bin/calagopus-minecraft-motd-agent
install -m 0644 "${script_dir}/calagopus-minecraft-motd-agent.service" \
  /etc/systemd/system/calagopus-minecraft-motd-agent.service

/usr/local/bin/calagopus-minecraft-motd-agent enroll \
  --panel-url "${panel_url}" \
  --enrollment-token "${enrollment_token}" \
  --listen-port "${listen_port}" \
  --output /etc/calagopus-motd-agent/config.toml

chmod 0600 /etc/calagopus-motd-agent/config.toml
systemctl daemon-reload
systemctl enable --now calagopus-minecraft-motd-agent.service
systemctl --no-pager --full status calagopus-minecraft-motd-agent.service

