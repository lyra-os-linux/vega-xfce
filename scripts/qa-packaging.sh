#!/usr/bin/env bash
# Validação estática reproduzível dos artefatos de pacote do vega-xfce.
#
# Desde a quebra do monorepo, vegad/vega-cli/vega-web/lyra-vega-dbus têm
# repositórios próprios (cada um com sua própria validação de
# empacotamento); este script cobre só o que ainda mora aqui.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

for command in desktop-file-validate xmllint rpmspec; do
  command -v "$command" >/dev/null || {
    echo "dependência ausente: $command" >&2
    exit 1
  }
done

desktop-file-validate packaging/vega/vega.desktop
xmllint --noout packaging/vega/icons/lyra-updates-symbolic.svg

rpmspec -P packaging/opensuse/vega.spec >/dev/null
rpmspec -P packaging/obs/vega-xfce.spec >/dev/null

# O Sobre deve receber a mesma versão declarada pelo RPM em qualquer build.
for spec in packaging/opensuse/vega.spec packaging/obs/vega-xfce.spec; do
  grep -q 'VEGA_VERSION=%{version} cargo build' "$spec"
done
grep -q 'VEGA_VERSION="\$VERSION" cargo build' packaging/opensuse/install.sh
grep -q 'option_env!("VEGA_VERSION")' vega-xfce/src/model.rs
grep -q '.version(crate::model::APPLICATION_VERSION)' vega-xfce/src/ui/shell.rs

echo "Empacotamento do vega-xfce: OK"
