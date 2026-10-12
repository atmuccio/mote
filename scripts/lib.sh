# shellcheck shell=bash
# Shared helpers for the Mote GitHub Action scripts. Source it; do not execute it.
#
# Portability: these scripts run under bash 3.2 (the /bin/bash on macOS runners), bash 5 (Linux) and
# Git Bash (Windows). So: no associative arrays, no mapfile/readarray, no ${var,,}, no ((i++)) under
# `set -e`, and empty arrays are expanded as ${arr[@]+"${arr[@]}"} (bash < 4.4 treats "${arr[@]}" of
# an empty array as unbound under `set -u`).
#
# Test hooks (see scripts/tests/run.sh):
#   MOTE_DRY_RUN=1                print GitHub API calls instead of making them
#   MOTE_DRY_RUN_LOG=<file>       append each call as a JSON line {method, path, body}
#   MOTE_DRY_RUN_FIXTURES=<dir>      canned responses: <METHOD>_<path with non-alnum as _>.json
#                                    and optional .status (HTTP status code, default 200/201)
#   MOTE_API_TRANSPORT=gh|curl       force the API client (default: gh when installed, else curl)
#
# Untrusted data (branch names, labels, file paths and messages from a PR) never reaches `eval`, a
# command line option position, or a workflow command unescaped: API path segments go through
# uri_encode, log output from mote is fenced with stop_commands/resume_commands, and workflow
# command data goes through wc_escape/wc_escape_prop.

MOTE_MARKER='<!-- mote:summary -->'

# --- platform ---------------------------------------------------------------------------------

is_windows() {
  case "${OSTYPE:-}" in msys* | cygwin* | win32*) return 0 ;; esac
  return 1
}

# jq.exe on Windows writes CRLF line endings unless -b is given; CRs would corrupt ids and URLs
# captured with $(...). Override jq for every script that sources this file.
if is_windows; then
  jq() { command jq -b "$@"; }
fi

# Convert a runner-provided path (may be C:\x\y on Windows) to one bash can use, and back.
to_unix_path() {
  if [ -n "$1" ] && command -v cygpath >/dev/null 2>&1; then cygpath -u "$1"; else printf '%s' "$1"; fi
}
to_native_path() {
  if [ -n "$1" ] && command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

# --- logging / workflow commands ------------------------------------------------------------------

# Escape data for a workflow command message (%, CR, LF).
wc_escape() {
  local s=$1 pct='%'
  s=${s//"$pct"/%25}
  s=${s//$'\r'/%0D}
  s=${s//$'\n'/%0A}
  printf '%s' "$s"
}
# Escape a workflow command property value (additionally ':' and ',').
wc_escape_prop() {
  local s
  s=$(wc_escape "$1")
  s=${s//:/%3A}
  s=${s//,/%2C}
  printf '%s' "$s"
}

log() { printf '%s\n' "$*" >&2; }
notice() { printf '::notice::%s\n' "$(wc_escape "$*")"; }
warn() { printf '::warning::%s\n' "$(wc_escape "$*")"; }
error() { printf '::error::%s\n' "$(wc_escape "$*")"; }
die() {
  error "$*"
  exit 1
}

# An unguessable token (hex): /dev/urandom, else $RANDOM.
random_token() {
  local t
  t=$(od -An -tx1 -N16 /dev/urandom 2>/dev/null | tr -d ' \n' || true)
  [ -n "$t" ] || t="${RANDOM}${RANDOM}${RANDOM}${RANDOM}${RANDOM}$$"
  printf '%s' "$t"
}

# Fence output that may contain text from the PR (mote prints file paths and messages): the runner
# ignores `::command::` lines between stop_commands and resume_commands. Keep everything that must be
# interpreted (warn/error/notice) outside the fence, and send the fenced program's stderr to stdout
# (2>&1) so both streams stay ordered relative to the fence lines.
MOTE_STOP_TOKEN=""
stop_commands() {
  MOTE_STOP_TOKEN="mote-$(random_token)"
  printf '::stop-commands::%s\n' "$MOTE_STOP_TOKEN"
}
resume_commands() {
  if [ -n "$MOTE_STOP_TOKEN" ]; then printf '::%s::\n' "$MOTE_STOP_TOKEN"; fi
  MOTE_STOP_TOKEN=""
}

# --- inputs / outputs -------------------------------------------------------------------------------

is_true() {
  case "$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')" in
    true | yes | y | 1 | on) return 0 ;;
  esac
  return 1
}

# set_output NAME VALUE — writes to $GITHUB_OUTPUT (multi-line safe); logs when not in Actions.
set_output() {
  local name=$1 value=${2:-} delim
  if [ -z "${GITHUB_OUTPUT:-}" ]; then
    log "output: $name=$value"
    return 0
  fi
  case "$value" in
    *$'\n'* | *$'\r'*)
      # The delimiter must not occur in the value, or the rest of it would be parsed as outputs.
      delim="MOTE_EOF_$(random_token)"
      while case "$value" in *"$delim"*) true ;; *) false ;; esac; do
        delim="MOTE_EOF_$(random_token)"
      done
      printf '%s<<%s\n%s\n%s\n' "$name" "$delim" "$value" "$delim" >>"$GITHUB_OUTPUT"
      ;;
    *) printf '%s=%s\n' "$name" "$value" >>"$GITHUB_OUTPUT" ;;
  esac
}

# A command-line operand that is a path must not be mistaken for an option: prefix '-…' with './'.
safe_path_arg() {
  case "$1" in -*) printf './%s' "$1" ;; *) printf '%s' "$1" ;; esac
}

# Normalize a relative directory for comparisons and keys: '\' -> '/', no leading './', no trailing
# '/', '' -> '.'.
normalize_rel_path() {
  local p=${1:-.}
  p=${p//\\//}
  while case "$p" in ./*) true ;; *) false ;; esac; do p=${p#./}; done
  while case "$p" in ?*/) true ;; *) false ;; esac; do p=${p%/}; done
  [ -n "$p" ] || p=.
  printf '%s' "$p"
}

sha256_of() { # FILE -> lowercase hex digest
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}

require_cmd() {
  local c
  for c in "$@"; do
    command -v "$c" >/dev/null 2>&1 || die "Required command '$c' not found on this runner."
  done
}

tmp_root() {
  local t=${RUNNER_TEMP:-${TMPDIR:-/tmp}}
  to_unix_path "$t"
}
mktemp_file() { mktemp "$(tmp_root)/mote.XXXXXX"; }
mktemp_dir() { mktemp -d "$(tmp_root)/mote.XXXXXX"; }

now_utc() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# --- event context ------------------------------------------------------------------------------

# event_get JQ_PATH — value from the webhook payload, empty when absent/null.
event_get() {
  local f=${GITHUB_EVENT_PATH:-}
  [ -n "$f" ] || return 0
  f=$(to_unix_path "$f")
  [ -f "$f" ] || return 0
  jq -r "($1) // empty" "$f" 2>/dev/null || true
}

is_pr_event() {
  case "${GITHUB_EVENT_NAME:-}" in
    pull_request | pull_request_target) [ -n "$(event_get .pull_request.number)" ] ;;
    *) return 1 ;;
  esac
}

repo_slug() {
  [ -n "${GITHUB_REPOSITORY:-}" ] || die "GITHUB_REPOSITORY is not set."
  printf '%s' "$GITHUB_REPOSITORY"
}

run_url() {
  printf '%s/%s/actions/runs/%s' "${GITHUB_SERVER_URL:-https://github.com}" "${GITHUB_REPOSITORY:-}" \
    "${GITHUB_RUN_ID:-0}"
}

# --- GitHub REST API ----------------------------------------------------------------------------

# GitHub Enterprise Server: `gh` talks to $GH_HOST and authenticates to an enterprise host with
# $GH_ENTERPRISE_TOKEN only ($GH_TOKEN is for github.com and ghe.com). Without both, every call would
# go out unauthenticated. An explicit GH_HOST / GH_ENTERPRISE_TOKEN from the workflow wins.
server_host() { # host[:port] of $GITHUB_SERVER_URL, lowercase
  local h=${GITHUB_SERVER_URL:-https://github.com}
  h=${h#*://}
  h=${h%%/*}
  printf '%s' "$h" | tr '[:upper:]' '[:lower:]'
}
case "$(server_host)" in
  github.com | '') ;;
  *)
    if [ -z "${GH_HOST:-}" ]; then
      GH_HOST=$(server_host)
      export GH_HOST
    fi
    if [ -z "${GH_ENTERPRISE_TOKEN:-}" ] && [ -n "${GH_TOKEN:-}" ]; then
      GH_ENTERPRISE_TOKEN=$GH_TOKEN
      export GH_ENTERPRISE_TOKEN
    fi
    ;;
esac

is_dry_run() { is_true "${MOTE_DRY_RUN:-}"; }

API_STATUS=""
API_ERROR=""
API_RESPONSE_FILE=""

# uri_encode STRING — percent-encode for use as ONE URL path segment (everything but A-Za-z0-9-_.~).
# Also neutralizes `gh api`'s {owner}/{repo}/{branch} placeholders.
uri_encode() { jq -rn --arg s "$1" '$s | @uri'; }
# uri_encode_path STRING — like uri_encode but keeps '/' (e.g. a branch name in a compare path).
uri_encode_path() { jq -rn --arg s "$1" '$s | @uri | gsub("%2F"; "/")'; }

api_transport() {
  case "${MOTE_API_TRANSPORT:-}" in
    gh | curl) printf '%s' "$MOTE_API_TRANSPORT" ;;
    *) if command -v gh >/dev/null 2>&1; then printf gh; else printf curl; fi ;;
  esac
}

# The curl fallback reads the Authorization header from a config on stdin (`curl --config -`): a
# token in argv would be visible to every process on the runner (ps, /proc/<pid>/cmdline).
_curl_auth_config() {
  [ -n "${GH_TOKEN:-}" ] || return 0
  local t
  # printf is a builtin and sed reads the token from the pipe: it never appears in an argv. Escape
  # '\' and '"' for curl's config syntax (tokens don't contain them, but the input is free text).
  t=$(printf '%s' "$GH_TOKEN" | tr -d '\r\n' | sed 's/[\\"]/\\&/g')
  printf 'header = "Authorization: Bearer %s"\n' "$t"
}

# api METHOD PATH [BODY_FILE]
# Response body -> $API_RESPONSE_FILE, HTTP status -> $API_STATUS. Returns 0 on 2xx, 1 otherwise.
# Uses `gh api` when available, else curl with $GH_TOKEN. Segments of PATH that come from outside
# (branch names, logins) must be passed through uri_encode first.
api() {
  local method=$1 path=$2 body=${3:-} errf
  API_RESPONSE_FILE=$(mktemp_file)
  API_STATUS=""
  API_ERROR=""

  if is_dry_run; then
    _api_dry_run "$method" "$path" "$body"
    case "$API_STATUS" in 2??) return 0 ;; esac
    API_ERROR="HTTP $API_STATUS (dry-run fixture)"
    return 1
  fi

  errf=$(mktemp_file)
  if [ "$(api_transport)" = gh ]; then
    local -a args
    args=(api --method "$method" -H 'Accept: application/vnd.github+json'
      -H 'X-GitHub-Api-Version: 2022-11-28' "$path")
    if [ -n "$body" ]; then args+=(--input "$body"); fi
    if gh "${args[@]}" >"$API_RESPONSE_FILE" 2>"$errf"; then
      API_STATUS=200
      rm -f "$errf"
      return 0
    fi
    API_STATUS=$(grep -Eo 'HTTP [0-9]{3}' "$errf" | tail -n 1 | cut -d' ' -f2 || true)
  else
    local base=${GITHUB_API_URL:-https://api.github.com}
    local -a cargs
    cargs=(--config - -sS -X "$method" -o "$API_RESPONSE_FILE" -w '%{http_code}'
      -H 'Accept: application/vnd.github+json' -H 'X-GitHub-Api-Version: 2022-11-28')
    if [ -n "$body" ]; then cargs+=(-H 'Content-Type: application/json' --data-binary "@$body"); fi
    API_STATUS=$(_curl_auth_config | curl "${cargs[@]}" "${base%/}/$path" 2>"$errf" || true)
    case "$API_STATUS" in 2??)
      rm -f "$errf"
      return 0
      ;;
    esac
  fi
  API_STATUS=${API_STATUS:-000}
  API_ERROR=$(head -c 2000 "$errf" 2>/dev/null || true)
  if [ -z "$API_ERROR" ] && [ -s "$API_RESPONSE_FILE" ]; then
    API_ERROR=$(jq -r '.message // empty' "$API_RESPONSE_FILE" 2>/dev/null || true)
  fi
  rm -f "$errf"
  return 1
}

_api_fixture_key() {
  # GET repos/o/r/issues/7/comments?per_page=100 -> GET_repos_o_r_issues_7_comments
  local p=${2%%\?*}
  printf '%s_%s' "$1" "$(printf '%s' "$p" | tr -c 'A-Za-z0-9._-' '_')"
}

_api_dry_run() {
  local method=$1 path=$2 body=$3 key fx size=""
  if [ -n "$body" ]; then size=" --input <$(wc -c <"$body" | tr -d ' ') bytes>"; fi
  log "[dry-run] gh api --method $method $path$size"
  if [ -n "${MOTE_DRY_RUN_LOG:-}" ]; then
    if [ -n "$body" ]; then
      jq -c --arg m "$method" --arg p "$path" '{method: $m, path: $p, body: .}' "$body" \
        >>"$MOTE_DRY_RUN_LOG"
    else
      jq -nc --arg m "$method" --arg p "$path" '{method: $m, path: $p, body: null}' \
        >>"$MOTE_DRY_RUN_LOG"
    fi
  fi
  key=$(_api_fixture_key "$method" "$path")
  fx=${MOTE_DRY_RUN_FIXTURES:-}
  if [ -n "$fx" ] && [ -f "$fx/$key.status" ]; then
    API_STATUS=$(tr -d ' \r\n' <"$fx/$key.status")
  elif [ "$method" = POST ]; then
    API_STATUS=201
  else
    API_STATUS=200
  fi
  if [ -n "$fx" ] && [ -f "$fx/$key.json" ]; then
    cat "$fx/$key.json" >"$API_RESPONSE_FILE"
  elif [ "$method" = POST ] || [ "$method" = PATCH ]; then
    printf '{"id": 4242, "html_url": "https://github.com/dry-run/4242"}' >"$API_RESPONSE_FILE"
  else
    printf '{}' >"$API_RESPONSE_FILE"
  fi
}

# api_list PATH OUT_FILE [MAX_PAGES] — GET every page of a list endpoint, merged into one JSON array.
api_list() {
  local path=$1 out=$2 max=${3:-30} page=1 sep n acc
  case "$path" in *\?*) sep='&' ;; *) sep='?' ;; esac
  acc=$(mktemp_file)
  printf '[]' >"$acc"
  while [ "$page" -le "$max" ]; do
    api GET "${path}${sep}per_page=100&page=${page}" || return 1
    jq -s '.[0] + (.[1] | if type == "array" then . else [] end)' "$acc" "$API_RESPONSE_FILE" \
      >"$acc.next"
    mv "$acc.next" "$acc"
    n=$(jq 'if type == "array" then length else 0 end' "$API_RESPONSE_FILE")
    [ "$n" -ge 100 ] || break
    page=$((page + 1))
  done
  mv "$acc" "$out"
}

is_permission_status() {
  case "${1:-$API_STATUS}" in 401 | 403 | 404) return 0 ;; esac
  return 1
}

# write_json_body FILE JQ_FILTER [jq args...] — build a request body with jq -n.
write_json_body() {
  local f=$1 filter=$2
  shift 2
  jq -n "$@" "$filter" >"$f"
}

# --- report composition -------------------------------------------------------------------------

# compose_report OUT_FILE MAX_BYTES — the Markdown used for the job summary and the PR comment:
# mote's Markdown (or a failure note), the override record, and a footer. Always begins with
# the hidden marker so the sticky comment can be found again. MOTE_ARTIFACT_NAME empty = no
# results artifact was uploaded (the footer doesn't mention one).
compose_report() {
  local out=$1 max=$2 md=${MOTE_MARKDOWN_FILE:-} body tail_note what artifact_note=""
  body=$(mktemp_file)
  what="the scan"
  case "${MOTE_MODE:-scan}" in [Gg][Aa][Tt][Ee]) what="the gate evaluation" ;; esac
  if [ -n "${MOTE_ARTIFACT_NAME:-}" ]; then
    artifact_note=$(printf ' · results artifact `%s`' "$MOTE_ARTIFACT_NAME")
  fi
  if [ -n "$md" ] && [ -s "$md" ]; then
    if [ "$(head -n 1 "$md" | tr -d '\r' | sed 's/^[[:space:]]*//')" != "$MOTE_MARKER" ]; then
      printf '%s\n' "$MOTE_MARKER" >"$body"
    fi
    cat "$md" >>"$body"
  else
    {
      printf '%s\n' "$MOTE_MARKER"
      printf '### Mote: %s did not complete\n\n' "$what"
      printf 'mote exited with code `%s` (2 = usage or configuration error, 3 = internal error).\n' \
        "${MOTE_EXIT_CODE:-?}"
      printf 'See the [workflow run log](%s) for details.\n' "$(run_url)"
    } >"$body"
  fi
  if [ -n "${MOTE_OVERRIDE_MD:-}" ] && [ -s "${MOTE_OVERRIDE_MD}" ]; then
    printf '\n' >>"$body"
    cat "$MOTE_OVERRIDE_MD" >>"$body"
  fi
  {
    printf '\n---\n'
    printf '<sub>Mote %s · [workflow run](%s)%s</sub>\n' "${MOTE_VERSION:-}" "$(run_url)" "$artifact_note"
  } >>"$body"

  if [ "$(wc -c <"$body" | tr -d ' ')" -le "$max" ]; then
    mv "$body" "$out"
    return 0
  fi
  if [ -n "${MOTE_ARTIFACT_NAME:-}" ]; then
    tail_note=$(printf '\n\n> [!NOTE]\n> Report truncated to fit GitHub limits. The full report is in the [workflow run](%s) (job summary and `%s` artifact).\n' \
      "$(run_url)" "$MOTE_ARTIFACT_NAME")
  else
    tail_note=$(printf '\n\n> [!NOTE]\n> Report truncated to fit GitHub limits. The full report is in the [workflow run](%s) (job summary).\n' \
      "$(run_url)")
  fi
  head -c "$((max - ${#tail_note} - 16))" "$body" >"$out"
  printf '%s\n' "$tail_note" >>"$out"
  rm -f "$body"
}
