#!/usr/bin/env bash
# Install the mote binary for this runner and put it on PATH.
#
# Inputs (env):
#   INPUT_MODE               the action's `mode` (validated here, the first step): scan|install|gate
#   INPUT_VERSION           explicit version ("1.4.2", "v1.4.2", "latest"); empty = derive from the action
#   INPUT_REPOSITORY         release source, owner/repo
#   INPUT_BUILD_FROM_SOURCE  "true" = cargo install instead of downloading a release
#   MOTE_ACTION_REF          github.action_ref (e.g. v1.4.2, v1, a SHA, main)
#   MOTE_ACTION_PATH         github.action_path (the action checkout; contains the source tree)
#   MOTE_RELEASE_BASE_URL  optional override of https://github.com/<repo>/releases/download/v<ver>
#                            (mirrors, tests; file:// works)
#   GH_TOKEN                 used by `gh release download` for private release repositories
# Outputs: version, bin (bash path to the binary), source (tool-cache|release|source)
#
# Release asset contract (must match .github/workflows/release.yml); <version> has no leading v:
#   mote-<version>-<target>.tar.gz | mote-<version>-<target>.zip (Windows)
#   + <archive>.sha256 ("<hex>  <archive>")
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"

# Expressions in action.yml compare strings case-insensitively; so does this check.
case "$(printf '%s' "${INPUT_MODE:-scan}" | tr '[:upper:]' '[:lower:]')" in
  scan | install | gate) ;;
  *) die "Invalid mode '${INPUT_MODE:-}': use scan, install or gate." ;;
esac

REPO=${INPUT_REPOSITORY:-atmuccio/mote}
printf '%s' "$REPO" | grep -Eqx '[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+' ||
  die "Invalid repository '$REPO': expected owner/repo."
case "/$REPO/" in */./* | */../*) die "Invalid repository '$REPO': expected owner/repo." ;; esac
SERVER=${GITHUB_SERVER_URL:-https://github.com}
SERVER=${SERVER%/}
ACTION_PATH=$(to_unix_path "${MOTE_ACTION_PATH:-${GITHUB_ACTION_PATH:-}}")

# --- target triple ----------------------------------------------------------------------------------
detect_target() {
  local os arch
  os=${RUNNER_OS:-$(uname -s)}
  arch=${RUNNER_ARCH:-$(uname -m)}
  case "$os" in
    Linux | linux) os=linux ;;
    macOS | Darwin | darwin) os=macos ;;
    Windows | MINGW* | MSYS* | CYGWIN*) os=windows ;;
    *) die "Unsupported runner OS: $os" ;;
  esac
  case "$arch" in
    X64 | x86_64 | amd64 | AMD64) arch=x86_64 ;;
    ARM64 | arm64 | aarch64) arch=aarch64 ;;
    *) die "Unsupported runner architecture: $arch" ;;
  esac
  ARCH=$arch
  EXE=""
  EXT=tar.gz
  case "$os" in
    linux) TARGET="$arch-unknown-linux-gnu" ;;
    macos) TARGET="$arch-apple-darwin" ;;
    windows)
      TARGET="$arch-pc-windows-msvc"
      EXE=.exe
      EXT=zip
      ;;
  esac
}

# --- version ----------------------------------------------------------------------------------------
semver_re='^v?[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.+-]+)?$'

workspace_version() { # the [workspace.package] version of a mote source tree
  local toml=$1/Cargo.toml
  [ -f "$toml" ] || return 0
  awk '
    /^\[/ { in_ws = ($0 ~ /^\[workspace\.package\]/) }
    in_ws && /^[[:space:]]*version[[:space:]]*=/ {
      gsub(/.*=[[:space:]]*"/, ""); gsub(/".*/, ""); print; exit
    }' "$toml" | tr -d '\r'
}

resolve_version() {
  VERSION_SOURCE=input
  local v=${INPUT_VERSION:-}
  if [ -z "$v" ]; then
    VERSION_SOURCE=action
    if [[ ${MOTE_ACTION_REF:-} =~ $semver_re ]]; then
      v=$MOTE_ACTION_REF # uses: atmuccio/mote@v1.4.2
    elif [ -n "$ACTION_PATH" ]; then
      v=$(workspace_version "$ACTION_PATH") # @v1, @<sha>, @main, uses: ./
    fi
  fi
  if [ -z "$v" ]; then
    VERSION_SOURCE=default
    v=latest
  fi
  v=${v#v}
  # The version becomes part of URLs, a tool-cache directory and the cache key: semver or latest.
  if [ "$v" != latest ] && ! [[ $v =~ $semver_re ]]; then
    die "Invalid version '$v': use X.Y.Z, vX.Y.Z or latest."
  fi
  if [ "$v" = latest ]; then
    # Resolve to a concrete tag: asset names carry the version, and the tool cache and the
    # analysis-cache key must be exact. The API first, then the web redirect (no API rate limit).
    local tag=""
    if api GET "repos/$REPO/releases/latest"; then
      tag=$(jq -r '.tag_name // empty' "$API_RESPONSE_FILE" 2>/dev/null || true)
    fi
    if [ -z "$tag" ]; then tag=$(latest_tag_from_redirect || true); fi
    if [ -n "$tag" ] && [[ $tag =~ $semver_re ]]; then
      v=${tag#v}
    elif ! is_true "${INPUT_BUILD_FROM_SOURCE:-false}"; then # a source build can use the default branch
      local why="API: HTTP ${API_STATUS:-none}"
      if [ -n "$tag" ]; then why="its tag '$tag' is not a version"; fi
      die "Could not resolve the latest release of $REPO ($why). Set 'version' to a release, e.g. 0.1.0."
    fi
  fi
  VERSION=$v
}

latest_tag_from_redirect() { # -> the tag <server>/<repo>/releases/latest redirects to
  local url
  if is_dry_run; then return 1; fi # tests: no network
  url=$(curl -sS -o /dev/null --retry 2 --connect-timeout 20 -w '%{redirect_url}' \
    "$SERVER/$REPO/releases/latest" 2>/dev/null) || return 1
  case "$url" in
    */releases/tag/?*) printf '%s' "${url##*/releases/tag/}" ;;
    *) return 1 ;;
  esac
}

# --- helpers ----------------------------------------------------------------------------------------
download() { # URL DEST
  curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 20 -o "$2" "$1"
}

extract() { # ARCHIVE DEST_DIR
  mkdir -p "$2"
  case "$1" in
    *.tar.gz) tar -xzf "$1" -C "$2" ;;
    *.zip)
      if command -v unzip >/dev/null 2>&1; then
        unzip -q -o "$1" -d "$2"
      elif command -v 7z >/dev/null 2>&1; then
        7z x -y -bd -o"$2" "$1" >/dev/null
      else
        # Paths go through the environment, never into the PowerShell source (quotes in a path).
        MOTE_ZIP=$(to_native_path "$1") MOTE_ZIP_DEST=$(to_native_path "$2") \
          powershell -NoProfile -NonInteractive -Command \
          'Expand-Archive -Force -LiteralPath $env:MOTE_ZIP -DestinationPath $env:MOTE_ZIP_DEST'
      fi
      ;;
    *) die "Unknown archive type: $1" ;;
  esac
}

tool_cache_dir() { # VERSION -> <tool cache>/mote/<version>/<arch> (the @actions/tool-cache layout)
  local root
  root=$(to_unix_path "${RUNNER_TOOL_CACHE:-}")
  [ -n "$root" ] || root="$(tmp_root)/tool-cache"
  printf '%s/mote/%s/%s' "$root" "$1" "$ARCH"
}

asset_name() { # the release archive for this runner: mote-<version>-<target>.<ext>
  printf 'mote-%s-%s.%s' "$VERSION" "$TARGET" "$EXT"
}

probe_version() { # BIN -> version reported by `mote version`, or empty
  "$1" version 2>/dev/null | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.+-]+)?' | head -n 1 || true
}

# --- install paths ----------------------------------------------------------------------------------
install_from_release() {
  local asset base work expected actual found
  if [ "$VERSION" = latest ]; then
    # Asset names carry the version, so an unresolved "latest" has nothing to download.
    warn "No concrete version of $REPO to download; set 'version' (e.g. 0.1.0)."
    return 1
  fi
  asset=$(asset_name)
  if [ -n "${MOTE_RELEASE_BASE_URL:-}" ]; then
    base=${MOTE_RELEASE_BASE_URL%/}
  else
    base="$SERVER/$REPO/releases/download/v$VERSION"
  fi
  work=$(mktemp_dir)
  log "Downloading $base/$asset"
  if ! download "$base/$asset" "$work/$asset" || ! download "$base/$asset.sha256" "$work/$asset.sha256"; then
    # Private release repositories need an authenticated download.
    if [ -z "${MOTE_RELEASE_BASE_URL:-}" ] && [ -n "${GH_TOKEN:-}" ] &&
      command -v gh >/dev/null 2>&1 &&
      gh release download "v$VERSION" -R "$REPO" -p "$asset" -p "$asset.sha256" -D "$work" --clobber; then
      :
    else
      return 1
    fi
  fi

  expected=$(awk 'NR == 1 {print $1}' "$work/$asset.sha256" | tr -d '*\r' | tr 'A-F' 'a-f')
  actual=$(sha256_of "$work/$asset")
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    # Never fall back to anything on a checksum mismatch.
    die "Checksum mismatch for $asset: expected '${expected:-<empty>}', got '$actual'."
  fi
  log "SHA-256 verified: $actual"

  extract "$work/$asset" "$work/x"
  found=$(find "$work/x" -type f -name "mote$EXE" | head -n 1)
  [ -n "$found" ] || die "$asset does not contain mote$EXE."
  chmod +x "$found"

  INSTALL_DIR=$(tool_cache_dir "$VERSION")
  mkdir -p "$INSTALL_DIR"
  cp "$found" "$INSTALL_DIR/mote$EXE"
  touch "$INSTALL_DIR.complete"
  SOURCE=release
}

install_from_source() {
  require_cmd cargo
  local dest local_version
  dest="$(tmp_root)/mote-from-source"
  local_version=""
  if [ -n "$ACTION_PATH" ] && [ -f "$ACTION_PATH/crates/mote-cli/Cargo.toml" ]; then
    local_version=$(workspace_version "$ACTION_PATH")
  fi
  if [ -n "$local_version" ] && { [ "$VERSION_SOURCE" != input ] || [ "$VERSION" = "$local_version" ]; }; then
    # The action checkout *is* the source tree at the pinned ref: build exactly that.
    log "Building mote from the action source ($ACTION_PATH, version $local_version)"
    cargo install --locked --path "$ACTION_PATH/crates/mote-cli" --root "$dest" --bin mote --force
    VERSION=$local_version
  else
    local -a args
    args=(install --locked --git "$SERVER/$REPO" --root "$dest" --bin mote --force)
    if [ "$VERSION" != latest ]; then args+=(--tag "v$VERSION"); fi
    args+=(mote-cli)
    log "cargo ${args[*]}"
    cargo "${args[@]}"
  fi
  INSTALL_DIR="$dest/bin"
  if [ "$VERSION" = latest ]; then
    local probed
    probed=$(probe_version "$INSTALL_DIR/mote$EXE")
    if [ -n "$probed" ]; then VERSION=$probed; fi
  fi
  SOURCE=source
}

# --- main -------------------------------------------------------------------------------------------
detect_target
resolve_version
log "mote: version=$VERSION (from $VERSION_SOURCE), target=$TARGET, repository=$REPO"

SOURCE=""
INSTALL_DIR=""
if is_true "${INPUT_BUILD_FROM_SOURCE:-false}"; then
  install_from_source
else
  cached=""
  if [ "$VERSION" != latest ]; then cached=$(tool_cache_dir "$VERSION"); fi
  if [ -n "$cached" ] && [ -f "$cached.complete" ] && [ -x "$cached/mote$EXE" ]; then
    INSTALL_DIR=$cached
    SOURCE=tool-cache
    log "Using cached mote from $INSTALL_DIR"
  elif ! install_from_release; then
    if command -v cargo >/dev/null 2>&1; then
      warn "No release asset $(asset_name) for $REPO; building from source (slow)."
      install_from_source
    else
      die "No release asset $(asset_name) for $REPO and cargo is not available. Set 'version' to a published release or 'build-from-source: true'."
    fi
  fi
fi

BIN="$INSTALL_DIR/mote$EXE"
[ -x "$BIN" ] || die "mote binary not found at $BIN"
if [ -n "${GITHUB_PATH:-}" ]; then
  printf '%s\n' "$(to_native_path "$INSTALL_DIR")" >>"$GITHUB_PATH"
fi
"$BIN" version >&2 || warn "'mote version' failed; continuing."

set_output version "$VERSION"
set_output bin "$BIN"
set_output source "$SOURCE"
