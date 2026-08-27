#!/usr/bin/env bash
# Reverses packaging/opensuse/install.sh (vega-xfce only — see that script
# for why vegad moved to its own repo/uninstaller).
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "Rode como root (sudo $0)" >&2
  exit 1
fi

echo "==> Removendo binários e app"
rm -f /usr/bin/vega-xfce
rm -f /usr/share/applications/vega-xfce.desktop
rm -f /usr/share/icons/hicolor/scalable/apps/vega.svg
rm -f /usr/share/icons/hicolor/symbolic/apps/lyra-updates-symbolic.svg

echo "vega-xfce removido."
