#!/usr/bin/env bash
# Smoke test local do vega-xfce. Desde a quebra do monorepo, vegad tem seu
# próprio smoke test (Go) no repositório dele; este cobre só o que ainda
# mora aqui.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[1/5] Rust formatting, tests and lints"
(cd "$repo_root/vega-xfce" && cargo fmt --check && cargo test --locked && cargo clippy --locked -- -D warnings)

echo "[2/5] Optimized GTK build"
(cd "$repo_root/vega-xfce" && cargo build --release --locked)

echo "[3/5] Packaging metadata"
bash -n "$repo_root/scripts/install.sh"
bash -n "$repo_root/packaging/opensuse/install.sh"

echo "[4/5] Native-package guard"
package_files=(
  "$repo_root/packaging/opensuse/vega.spec"
  "$repo_root/packaging/obs"/*.spec
)
if grep -Ei '(electron|node_modules|npm (ci|install|run)|nodejs)' "${package_files[@]}"; then
  echo "Erro: referência ao runtime legado no pacote GTK" >&2
  exit 1
fi
# O empacotamento openSUSE usa o rust/cargo fornecido pelo próprio zypper.

echo "[5/5] Identidade XFCE"
grep -q 'vega-xfce' "$repo_root/vega-xfce/Cargo.toml"

echo "Smoke local concluído"
