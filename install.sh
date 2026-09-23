#!/bin/sh
set -eu

MANIFEST=${FILEBLADE_INSTALL_MANIFEST:-https://github.com/data-goblin/fileblade/releases/latest/download/latest.json}

fail() {
  printf 'fileblade install: %s\n' "$*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "required command unavailable: $1"
}

fetch() {
  case $1 in
    /* | file://*)
      local_path=${1#file://}
      [ -f "$local_path" ] || return 1
      cat -- "$local_path" > "$2"
      return 0
      ;;
  esac
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --proto '=https' --tlsv1.2 -o "$2" -- "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" -- "$1"
  else
    fail 'neither curl nor wget is available'
  fi
}

digest() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -- "$1" | cut -d ' ' -f 1
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 -- "$1" | sed 's/.*= //'
  else
    fail 'no SHA-256 tool is available (sha256sum, shasum or openssl)'
  fi
}

field() {
  jq -er --arg target "$1" --arg key "$2" '.artifacts[$target][$key]' "$3" 2>/dev/null
}

[ "$(uname -s)" = Linux ] || fail 'FileBlade runs on Linux with Hyprland; this system is not Linux'

case $(uname -m) in
  x86_64) machine=x86_64 ;;
  aarch64 | arm64) machine=aarch64 ;;
  *) fail "unsupported architecture: $(uname -m)" ;;
esac
target="$machine-unknown-linux-musl"

need jq
need tar
need install

work=$(mktemp -d "${TMPDIR:-/tmp}/fileblade-install.XXXXXX") || fail 'cannot create a working directory'
trap 'rm -rf -- "$work"' EXIT INT TERM
chmod 700 "$work"

printf 'Reading %s\n' "$MANIFEST"
fetch "$MANIFEST" "$work/latest.json" || fail 'cannot read the release manifest'

jq -e '.schema == 1' "$work/latest.json" >/dev/null 2>&1 || fail 'unsupported release manifest schema'
version=$(jq -er '.version' "$work/latest.json" 2>/dev/null) || fail 'the manifest declares no version'
url=$(field "$target" url "$work/latest.json") || fail "the manifest carries no $target artifact"
want=$(field "$target" sha256 "$work/latest.json") || fail "the manifest carries no $target checksum"

case $url in
  https://*) ;;
  /* | file://*)
    case $MANIFEST in
      /* | file://*) ;;
      *) fail 'a remote manifest may not point at a local artifact' ;;
    esac
    ;;
  *) fail 'the manifest artifact URL is not https' ;;
esac
case $want in
  '' | *[!0-9a-f]*) fail 'the manifest checksum is not a SHA-256 digest' ;;
esac
[ ${#want} -eq 64 ] || fail 'the manifest checksum is not a SHA-256 digest'

printf 'Downloading FileBlade %s for %s\n' "$version" "$target"
fetch "$url" "$work/payload.tar.gz" || fail 'cannot download the runtime archive'

got=$(digest "$work/payload.tar.gz")
[ "$got" = "$want" ] || fail "checksum mismatch: expected $want, got $got"

mkdir "$work/payload"
tar -xzf "$work/payload.tar.gz" -C "$work/payload" || fail 'cannot extract the runtime archive'
root=$work/payload
[ -f "$root/payload.json" ] || root=$work/payload/payload
[ -f "$root/payload.json" ] || fail 'the archive does not carry a FileBlade runtime payload'
jq -e --arg version "$version" --arg target "$target" '.version == $version and .target == $target' "$root/payload.json" >/dev/null 2>&1 \
  || fail 'the archive version or target differs from the release manifest'
[ -x "$root/tools/native" ] || fail 'the archive does not carry its installer'

printf 'Verifying the payload inventory\n'
"$root/tools/native" verify "$root" || fail 'the downloaded payload failed verification'

printf 'Installing\n'
"$root/tools/native" install "$root"

printf '\nFileBlade %s is installed.\n' "$version"
printf 'Run: fileblade --help\n'
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) printf 'Add %s to PATH: export PATH="%s:$PATH"\n' "$HOME/.local/bin" "$HOME/.local/bin" ;;
esac
