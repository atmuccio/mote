#!/usr/bin/env bash
# Mote speed benchmark (docs/benchmark.md, part 3).
#
# Fetches the pinned public repositories listed in bench/corpora.toml (shallow, one commit each),
# scans every corpus cold (empty cache directory) and warm (cache primed) --runs times, and prints
# a Markdown table: files, lines, findings, min/median wall time, lines per second.
#
# Usage: scripts/bench.sh [options]
#   --bin PATH          mote binary (default: target/release/mote[.exe])
#   --corpora FILE      corpus list (default: bench/corpora.toml)
#   --only NAMES        comma-separated corpus names (default: all)
#   --runs N            timed runs per mode (default: 5)
#   --threads N         worker threads passed to mote (default: all cores)
#   --work-dir DIR      where corpora are fetched (default: $MOTE_BENCH_DIR or bench/.corpora)
#   --reuse DIR         use DIR/<name> when it already holds the pinned commit (never modified;
#                       also measured as a working tree when HEAD is the pinned commit and clean)
#   --checkout          check out fetched corpora too and measure working-tree scans
#   --no-fetch          fail instead of fetching a missing corpus (offline runs)
#   --thresholds FILE   compare medians with the baselines in FILE; exit 1 on a regression
#   --machine NAME      baseline table in the thresholds file (default: $MOTE_BENCH_MACHINE)
#   --output FILE       also write the Markdown report to FILE
#
# Wall time is measured around the whole mote process (start-up and writing the JSON results
# included). "Cold" means an empty analysis cache, not a cold OS file cache: an untimed run warms
# the file cache (and primes the warm cache) before the timed runs.
#
# Exit status: 0 done, 1 a median regressed beyond the threshold, 2 usage or setup error.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

bin=""
corpora="$root/bench/corpora.toml"
only=""
runs=5
threads=""
work="${MOTE_BENCH_DIR:-$root/bench/.corpora}"
reuse=""
checkout=0
fetch=1
thresholds=""
machine="${MOTE_BENCH_MACHINE:-}"
output=""

log() { printf 'bench: %s\n' "$*" >&2; }
die() {
  printf 'bench: error: %s\n' "$*" >&2
  exit 2
}
usage() {
  sed -n '/^# Usage:/,/^# Wall time/p' "${BASH_SOURCE[0]}" | sed -e '$d' -e 's/^# \{0,1\}//'
}

need_value() {
  if [ "$#" -lt 2 ] || [ -z "$2" ]; then die "$1 needs a value"; fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --bin) need_value "$@"; bin=$2; shift 2 ;;
    --corpora) need_value "$@"; corpora=$2; shift 2 ;;
    --only) need_value "$@"; only=$2; shift 2 ;;
    --runs) need_value "$@"; runs=$2; shift 2 ;;
    --threads) need_value "$@"; threads=$2; shift 2 ;;
    --work-dir) need_value "$@"; work=$2; shift 2 ;;
    --reuse) need_value "$@"; reuse=$2; shift 2 ;;
    --checkout) checkout=1; shift ;;
    --no-fetch) fetch=0; shift ;;
    --thresholds) need_value "$@"; thresholds=$2; shift 2 ;;
    --machine) need_value "$@"; machine=$2; shift 2 ;;
    --output) need_value "$@"; output=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; die "unknown argument: $1" ;;
  esac
done

case "$runs" in '' | *[!0-9]* | 0) die "--runs must be a positive integer" ;; esac
if [ -n "$threads" ]; then
  case "$threads" in *[!0-9]* | 0) die "--threads must be a positive integer" ;; esac
fi
[ -f "$corpora" ] || die "no corpus list at $corpora"
if [ -n "$thresholds" ]; then
  [ -f "$thresholds" ] || die "no thresholds file at $thresholds"
  [ -n "$machine" ] || die "--thresholds needs --machine NAME (or MOTE_BENCH_MACHINE)"
fi
if [ -z "$bin" ]; then
  bin="$root/target/release/mote"
  [ -x "$bin" ] || bin="$root/target/release/mote.exe"
fi
[ -x "$bin" ] || die "mote binary not found at $bin; build it with: cargo build --release -p mote-cli"
command -v git >/dev/null 2>&1 || die "git is required"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/mote-bench.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
result_json="$scratch/result.json"
mote_log="$scratch/mote.log"

# --- bench/corpora.toml: one "name|url|rev|language" line per [[corpus]] table ------------------
parse_corpora() {
  awk '
    function flush() { if (name != "") print name "|" url "|" rev "|" lang; name = url = rev = lang = "" }
    /^[[:space:]]*(#|$)/ { next }
    /^[[:space:]]*\[\[corpus\]\]/ { flush(); next }
    /^[[:space:]]*[A-Za-z_]+[[:space:]]*=/ {
      key = $0; sub(/[[:space:]]*=.*/, "", key); gsub(/[[:space:]]/, "", key)
      val = $0; sub(/^[^=]*=[[:space:]]*"/, "", val); sub(/"[[:space:]]*(#.*)?$/, "", val)
      if (key == "name") name = val
      else if (key == "url") url = val
      else if (key == "rev") rev = val
      else if (key == "language") lang = val
    }
    END { flush() }
  ' "$1"
}

selected() { # name
  [ -z "$only" ] && return 0
  case ",$only," in *",$1,"*) return 0 ;; esac
  return 1
}

# --- corpora -----------------------------------------------------------------------------------
has_commit() { # dir rev
  [ -e "$1/.git" ] && git -C "$1" cat-file -e "$2^{commit}" 2>/dev/null
}

has_clean_checkout() { # dir rev
  local head
  head=$(git -C "$1" rev-parse -q --verify HEAD 2>/dev/null) || return 1
  [ "$head" = "$2" ] || return 1
  [ -z "$(git -C "$1" status --porcelain --untracked-files=no 2>/dev/null | head -n 1)" ]
}

corpus_dir() { # name url rev -> prints the directory holding rev
  local name=$1 url=$2 rev=$3 dir
  if [ -n "$reuse" ] && has_commit "$reuse/$name" "$rev"; then
    printf '%s\n' "$reuse/$name"
    return
  fi
  dir="$work/$name"
  if ! has_commit "$dir" "$rev"; then
    [ "$fetch" = 1 ] || die "corpus $name is not available at $rev in $dir (and --no-fetch was given)"
    mkdir -p "$dir"
    [ -e "$dir/.git" ] || git -C "$dir" init -q
    if git -C "$dir" remote get-url origin >/dev/null 2>&1; then
      git -C "$dir" remote set-url origin "$url"
    else
      git -C "$dir" remote add origin "$url"
    fi
    log "fetching $name at ${rev:0:12} (depth 1) from $url"
    git -C "$dir" fetch -q --depth 1 --no-tags origin "$rev" >&2 || die "could not fetch $rev from $url"
  fi
  if [ "$checkout" = 1 ] && ! has_clean_checkout "$dir" "$rev"; then
    log "checking out $name at ${rev:0:12}"
    git -C "$dir" -c advice.detachedHead=false checkout -q --force --detach "$rev" >&2
  fi
  printf '%s\n' "$dir"
}

# --- measuring ---------------------------------------------------------------------------------
# Runs mote with the given arguments, sets ELAPSED (seconds, 3 decimals) and returns its exit status.
ELAPSED=""
timed_scan() {
  local timefile="$scratch/time" rc=0
  local TIMEFORMAT=%3R
  { time "$bin" "$@" >/dev/null 2>>"$mote_log"; } 2>"$timefile" || rc=$?
  ELAPSED=$(tail -n 1 "$timefile")
  return "$rc"
}

# Prints "files lines ncloc findings files_analyzed cache_hits" from a pretty-printed results JSON.
json_numbers() {
  awk '
    /^  "measures": \{/ { m = 1; next }
    m && /^    "total": \{/ { t = 1; next }
    t && /^    \}/ { t = 0; m = 0 }
    t && /^      "(files|lines|ncloc|findings)": [0-9]/ { s = $0; gsub(/[ ",]/, "", s); split(s, kv, ":"); v[kv[1]] = kv[2] }
    /^  "stats": \{/ { st = 1; next }
    st && /^  \}/ { st = 0 }
    st && /^    "(files_analyzed|cache_hits)": [0-9]/ { s = $0; gsub(/[ ",]/, "", s); split(s, kv, ":"); v[kv[1]] = kv[2] }
    END { printf "%s %s %s %s %s %s\n", v["files"] + 0, v["lines"] + 0, v["ncloc"] + 0, v["findings"] + 0, v["files_analyzed"] + 0, v["cache_hits"] + 0 }
  ' "$1"
}

min_median() { # times... -> "min median"
  printf '%s\n' "$@" | sort -n | awk '
    { a[NR] = $1 }
    END { med = (NR % 2) ? a[(NR + 1) / 2] : (a[NR / 2] + a[NR / 2 + 1]) / 2; printf "%.3f %.3f\n", a[1], med }'
}

fmt_int() { # 1234567 -> 1,234,567
  awk -v n="$1" 'BEGIN { s = sprintf("%d", n); o = ""; while (length(s) > 3) { o = "," substr(s, length(s) - 2) o; s = substr(s, 1, length(s) - 3) } print s o }'
}

fmt_secs() { awk -v s="$1" 'BEGIN { printf((s < 10) ? "%.2f s" : "%.1f s", s) }'; }

fmt_rate() { # lines seconds -> "2.3M" / "850k"
  awk -v l="$1" -v s="$2" 'BEGIN { if (s <= 0) { print "-"; exit } r = l / s; if (r >= 1e6) printf "%.1fM\n", r / 1e6; else printf "%.0fk\n", r / 1e3 }'
}

# One benchmark: prints a Markdown row, appends "key median" lines to $scratch/medians.
bench_one() { # name language dir rev source(rev|worktree)
  local name=$1 lang=$2 dir=$3 rev=$4 source=$5
  local -a args=(scan "$dir" --no-gate -q --format json -o "$result_json")
  [ "$source" = rev ] && args+=(--rev "$rev")
  [ -n "$threads" ] && args+=(--threads "$threads")
  local warm_cache="$scratch/cache-warm" cold_cache="$scratch/cache-cold"
  local i cold=() warm=() numbers files lines findings analyzed

  rm -rf "$warm_cache"
  log "$name ($source): priming run"
  timed_scan "${args[@]}" --cache-dir "$warm_cache" || die "mote failed on $name ($source); see output above"
  numbers=$(json_numbers "$result_json")
  read -r files lines _ findings _ _ <<<"$numbers"

  for ((i = 1; i <= runs; i++)); do
    rm -rf "$cold_cache"
    timed_scan "${args[@]}" --cache-dir "$cold_cache" || die "mote failed on $name ($source)"
    cold+=("$ELAPSED")
  done
  for ((i = 1; i <= runs; i++)); do
    timed_scan "${args[@]}" --cache-dir "$warm_cache" || die "mote failed on $name ($source)"
    warm+=("$ELAPSED")
  done
  rm -rf "$cold_cache" "$warm_cache"
  read -r _ _ _ _ analyzed _ <<<"$(json_numbers "$result_json")"
  [ "$analyzed" = 0 ] || log "warning: a warm scan of $name ($source) still analyzed $analyzed files"

  local cold_min cold_med warm_min warm_med
  read -r cold_min cold_med <<<"$(min_median "${cold[@]}")"
  read -r warm_min warm_med <<<"$(min_median "${warm[@]}")"
  log "$name ($source): cold ${cold[*]} | warm ${warm[*]}"
  printf '%s.%s.cold %s\n%s.%s.warm %s\n' "$name" "$source" "$cold_med" "$name" "$source" "$warm_med" >>"$scratch/medians"
  printf '| %s | %s | `%s` %s | %s | %s | %s | %s | %s | %s | %s | %s/s |\n' \
    "$name" "$lang" "${rev:0:10}" "$source" "$(fmt_int "$files")" "$(fmt_int "$lines")" \
    "$(fmt_int "$findings")" "$(fmt_secs "$cold_min")" "$(fmt_secs "$cold_med")" \
    "$(fmt_secs "$warm_min")" "$(fmt_secs "$warm_med")" "$(fmt_rate "$lines" "$cold_med")"
}

# --- thresholds ----------------------------------------------------------------------------------
threshold_value() { # key [section] -> value (a section of "" means the top level)
  awk -v want="$1" -v section="${2:-}" '
    /^[[:space:]]*\[/ { s = $0; sub(/#.*/, "", s); gsub(/[][[:space:]]/, "", s); cur = s; next }
    {
      line = $0; sub(/#.*/, "", line)
      if (cur != section || line !~ /=/) next
      key = line; sub(/[[:space:]]*=.*/, "", key); gsub(/^[[:space:]]*"?|"?[[:space:]]*$/, "", key)
      val = line; sub(/^[^=]*=[[:space:]]*/, "", val); gsub(/[[:space:]"]/, "", val)
      if (key == want) { print val; exit }
    }
  ' "$thresholds"
}

check_thresholds() {
  local pct key median base limit failed=0 checked=0
  pct=$(threshold_value max_regression_pct "machine.$machine")
  [ -n "$pct" ] || pct=$(threshold_value max_regression_pct "")
  [ -n "$pct" ] || pct=25
  while read -r key median; do
    base=$(threshold_value "$key" "machine.$machine")
    [ -n "$base" ] || continue
    checked=$((checked + 1))
    limit=$(awk -v b="$base" -v p="$pct" 'BEGIN { printf "%.3f", b * (1 + p / 100) }')
    if awk -v m="$median" -v l="$limit" 'BEGIN { exit !(m > l) }'; then
      printf -- '- **Regression** `%s`: median %s s > %s s (baseline %s s + %s%%)\n' "$key" "$median" "$limit" "$base" "$pct"
      failed=1
    else
      printf -- '- ok `%s`: median %s s <= %s s (baseline %s s + %s%%)\n' "$key" "$median" "$limit" "$base" "$pct"
    fi
  done <"$scratch/medians"
  if [ "$checked" = 0 ]; then
    printf -- '- No baselines for machine `%s` in %s; nothing checked.\n' "$machine" "${thresholds#"$root"/}"
  fi
  return "$failed"
}

# --- main ----------------------------------------------------------------------------------------
cpu_model() {
  if [ -r /proc/cpuinfo ]; then
    awk -F': *' '/^model name/ { print $2; exit }' /proc/cpuinfo
  elif command -v sysctl >/dev/null 2>&1; then
    sysctl -n machdep.cpu.brand_string 2>/dev/null || true
  fi
}
cpu_count() {
  getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo "?"
}

: >"$scratch/medians"
rows=()
found=0
while IFS='|' read -r name url rev lang; do
  selected "$name" || continue
  found=$((found + 1))
  if [ -z "$url" ] || [ -z "$rev" ]; then die "corpus $name in $corpora needs url and rev"; fi
  dir=$(corpus_dir "$name" "$url" "$rev")
  rows+=("$(bench_one "$name" "$lang" "$dir" "$rev" rev)")
  if has_clean_checkout "$dir" "$rev"; then
    rows+=("$(bench_one "$name" "$lang" "$dir" "$rev" worktree)")
  fi
done < <(parse_corpora "$corpora")
[ "$found" -gt 0 ] || die "no corpus selected (--only ${only:-<all>})"

report="$scratch/report.md"
{
  printf '### Mote speed benchmark\n\n'
  printf -- '- %s\n' "$("$bin" version | head -n 1), $(date -u '+%Y-%m-%d %H:%M UTC')"
  printf -- '- %s; %s logical CPUs; %s\n' "$(cpu_model)" "$(cpu_count)" "$(uname -srm)"
  printf -- '- %s timed runs per mode%s; wall time of the whole process, JSON results written to a file\n\n' \
    "$runs" "${threads:+, $threads threads}"
  printf '| Corpus | Lang | Source | Files | Lines | Findings | Cold min | Cold median | Warm min | Warm median | Lines/s (cold) |\n'
  printf '|---|---|---|--:|--:|--:|--:|--:|--:|--:|--:|\n'
  printf '%s\n' "${rows[@]}"
  printf '\nSource: `rev` scans the commit from the object database (`--rev`); `worktree` scans the checkout.\n'
  printf 'Cold: empty cache directory. Warm: cache primed by an earlier run of the same scan.\n'
} >"$report"

status=0
if [ -n "$thresholds" ]; then
  {
    printf '\n#### Regression check (machine `%s`)\n\n' "$machine"
    check_thresholds || status=1
  } >>"$report"
fi

cat "$report"
if [ -n "$output" ]; then
  cp "$report" "$output"
fi
if [ "$status" != 0 ]; then
  log "median wall time regressed beyond the threshold"
fi
exit "$status"
