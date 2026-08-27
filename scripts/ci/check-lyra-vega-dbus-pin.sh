#!/usr/bin/env bash
# Garante que o Cargo.lock reflete a tag de lyra-vega-dbus fixada em
# vega-xfce/Cargo.toml. Protege contra um pin atualizado sem `cargo update`
# (ou vice-versa), que faria o CI compilar contra um contrato D-Bus
# diferente do declarado.
set -euo pipefail

repo_root="$(cd "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"

tag="$(grep -oP 'lyra-vega-dbus = \{ git = "[^"]+", tag = "\K[^"]+' "$repo_root/vega-xfce/Cargo.toml")"
if [ -z "$tag" ]; then
  echo "não foi possível localizar a tag fixada de lyra-vega-dbus em vega-xfce/Cargo.toml" >&2
  exit 1
fi

if ! grep -q "lyra-vega-dbus?tag=${tag}#" "$repo_root/Cargo.lock"; then
  echo "Cargo.lock não reflete a tag fixada de lyra-vega-dbus (${tag})." >&2
  echo "Rode 'cargo update -p lyra-vega-dbus --precise <rev>' ou ajuste o pin em vega-xfce/Cargo.toml e recomite o Cargo.lock." >&2
  exit 1
fi

echo "lyra-vega-dbus fixado em ${tag} e refletido no Cargo.lock."
