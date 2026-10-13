#!/bin/sh
# Install a Mote release binary: download the archive for this machine, verify its SHA-256, extract.
#
#   MOTE_VERSION=0.2.0 sh install-mote.sh [DEST_DIR]      # DEST_DIR defaults to ./.mote-bin
#   export PATH="$PWD/.mote-bin:$PATH"
#
# Copy this file into your repository rather than piping it from the network. It prints the path of
# the installed binary. Needs curl, tar and sha256sum (or shasum); on Windows (Git Bash) unzip or
# PowerShell. Optional: MOTE_REPOSITORY (default atmuccio/mote); MOTE_RELEASE_URL, a directory URL
# that holds the archives instead of the GitHub release (an internal mirror, file:// works);
# MOTE_LIBC=musl for the static Linux build (chosen automatically on Alpine; only when the release
# has one).
set -eu

version=${MOTE_VERSION:-}
if [ -z "$version" ]; then
  echo "install-mote: set MOTE_VERSION, e.g. MOTE_VERSION=0.2.0" >&2
  exit 2
fi
version=${version#v}
repo=${MOTE_REPOSITORY:-atmuccio/mote}
dest=${1:-.mote-bin}

arch=$(uname -m)
case "$arch" in
  x86_64 | amd64 | AMD64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *)
    echo "install-mote: unsupported architecture $arch" >&2
    exit 2
    ;;
esac
exe=""
ext=tar.gz
case "$(uname -s)" in
  Linux)
    libc=${MOTE_LIBC:-gnu}
    if [ -z "${MOTE_LIBC:-}" ] && [ -f /etc/alpine-release ]; then libc=musl; fi
    target="$arch-unknown-linux-$libc"
    ;;
  Darwin) target="$arch-apple-darwin" ;;
  MINGW* | MSYS* | CYGWIN*)
    target="$arch-pc-windows-msvc"
    exe=.exe
    ext=zip
    ;;
  *)
    echo "install-mote: unsupported operating system $(uname -s)" >&2
    exit 2
    ;;
esac

archive="mote-$version-$target.$ext"
url=${MOTE_RELEASE_URL:-https://github.com/$repo/releases/download/v$version}
url=${url%/}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "install-mote: downloading $url/$archive" >&2
curl -fsSL --retry 3 --connect-timeout 20 -o "$work/$archive" "$url/$archive"
curl -fsSL --retry 3 --connect-timeout 20 -o "$work/$archive.sha256" "$url/$archive.sha256"

expected=$(awk 'NR == 1 { print $1 }' "$work/$archive.sha256" | tr -d '*\r' | tr 'A-F' 'a-f')
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$work/$archive" | awk '{ print $1 }')
else
  actual=$(shasum -a 256 "$work/$archive" | awk '{ print $1 }')
fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "install-mote: checksum mismatch for $archive (expected '$expected', got '$actual')" >&2
  exit 1
fi

mkdir -p "$dest"
case "$ext" in
  tar.gz) tar -xzf "$work/$archive" -C "$dest" "mote$exe" ;;
  zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q -o "$work/$archive" "mote$exe" -d "$dest"
    else
      # shellcheck disable=SC2016 # on purpose: PowerShell expands $env:…, not the shell
      MOTE_ZIP=$(cygpath -w "$work/$archive") MOTE_DEST=$(cygpath -w "$dest") \
        powershell -NoProfile -NonInteractive -Command \
        'Expand-Archive -Force -LiteralPath $env:MOTE_ZIP -DestinationPath $env:MOTE_DEST'
    fi
    ;;
esac
chmod +x "$dest/mote$exe"
"$dest/mote$exe" version >&2
printf '%s\n' "$dest/mote$exe"
