#!/usr/bin/env bash
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
  echo "Run this uninstaller as root." >&2
  exit 1
fi

systemctl stop calagopus-minecraft-motd-agent.service 2>/dev/null || true
systemctl disable calagopus-minecraft-motd-agent.service 2>/dev/null || true
rm -f /etc/systemd/system/calagopus-minecraft-motd-agent.service
rm -f /usr/local/bin/calagopus-minecraft-motd-agent
systemctl daemon-reload

if [[ ${1:-} == "--purge" ]]; then
  rm -f /etc/calagopus-motd-agent/config.toml
  rmdir /etc/calagopus-motd-agent 2>/dev/null || true
  echo "Agent, service, and credentials removed."
else
  echo "Agent and service removed. Credentials remain in /etc/calagopus-motd-agent/."
fi

