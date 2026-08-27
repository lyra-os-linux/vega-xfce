#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
samples="${1:-10}"
settle_seconds="${VEGA_BENCHMARK_SETTLE_SECONDS:-5}"
output="${VEGA_BENCHMARK_OUTPUT:-$repo_root/docs/migration/rust-gtk-benchmark.csv}"
binary="$repo_root/target/release/vega-xfce"
cargo_bin="${CARGO:-cargo}"

[[ "$samples" =~ ^[1-9][0-9]*$ ]] || { echo "Amostras deve ser inteiro positivo" >&2; exit 2; }
(cd "$repo_root/vega-xfce" && "$cargo_bin" build --release --locked)

printf 'sample,startup_ms,pss_kib,processes,cpu_percent,binary_bytes\n' >"$output"
for ((sample = 1; sample <= samples; sample++)); do
  log="$(mktemp)"
  VEGA_BENCHMARK_MARKER=1 "$binary" 2>"$log" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null || true; rm -f "$log"' EXIT

  startup_ms=""
  for _ in {1..200}; do
    startup_ms="$(sed -n 's/^VEGA_WINDOW_READY_MS=//p' "$log" | tail -n 1)"
    [[ -n "$startup_ms" ]] && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.05
  done
  [[ -n "$startup_ms" ]] || { cat "$log" >&2; echo "UI não abriu" >&2; exit 1; }

  sleep "$settle_seconds"
  pss_kib="$(awk '/^Pss:/ {sum += $2} END {print sum + 0}' /proc/"$pid"/smaps_rollup)"
  processes="$(pgrep -P "$pid" 2>/dev/null | wc -l || true)"
  processes=$((processes + 1))
  ticks_before="$(awk '{print $14 + $15}' /proc/"$pid"/stat)"
  sleep 1
  ticks_after="$(awk '{print $14 + $15}' /proc/"$pid"/stat)"
  hz="$(getconf CLK_TCK)"
  cpu_percent="$(awk -v delta="$((ticks_after - ticks_before))" -v hz="$hz" 'BEGIN {printf "%.2f", delta * 100 / hz}')"
  binary_bytes="$(stat -c %s "$binary")"
  printf '%s,%s,%s,%s,%s,%s\n' "$sample" "$startup_ms" "$pss_kib" "$processes" "$cpu_percent" "$binary_bytes" >>"$output"

  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -f "$log"
  trap - EXIT
done

echo "Métricas gravadas em $output"
