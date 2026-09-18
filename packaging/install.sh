private_directory() {
  [[ ! -L $1 ]] || fail "installation directory is a symlink: $1"
  if [[ ! -e $1 ]]; then mkdir -m 700 -- "$1"; fi
  [[ -d $1 && $(stat -c %u:%a -- "$1") == "$(id -u):700" ]] || fail "installation directory must be owned and private: $1"
}

launcher_text() {
  printf '#!/usr/bin/env bash\nset -euo pipefail\n'
  printf 'installation=%q\n' "$installation"
  printf '%s\n' \
    'exec 9>"$installation/lock"' \
    'flock -s 9' \
    'runtime=$(readlink -f -- "$installation/active/runtime")' \
    '[[ $runtime == "$installation/versions/"* && -x $runtime/app/launch ]] || { printf "%s\n" "fileblade: activation unavailable; use tools/native rollback" >&2; exit 1; }' \
    'export FILEBLADE_APP_ROOT=$runtime' \
    'exec "$runtime/app/launch" "$@"'
}

installation_paths() {
  [[ ${HOME:-} == /* && $HOME != / ]] || fail 'HOME must be an absolute user directory'
  local data=${XDG_DATA_HOME:-$HOME/.local/share}
  [[ $data == /* ]] || data=$HOME/.local/share
  installation=$data/fileblade/installation
  launcher=$HOME/.local/bin/fileblade
  [[ $(realpath -m -- "$installation") == "$installation" ]] || fail 'installation path contains a symlink or noncanonical component'
  [[ $(realpath -m -- "$HOME/.local/bin") == "$HOME/.local/bin" ]] || fail 'launcher parent contains a symlink or noncanonical component'
}

check_owner() {
  local path package
  for path in /usr/bin/fileblade /usr/bin/fileblade-bin /usr/lib/fileblade "$launcher"; do
    if [[ -e $path || -L $path ]]; then
      package=$(pacman -Qoq -- "$path" 2>/dev/null) && fail "package-owned installation ($package): use pacman"
    fi
  done
  if [[ -e $launcher || -L $launcher ]]; then
    [[ -L $launcher && $(readlink -- "$launcher") == "$installation/launcher" ]] || fail "unrelated file occupies $launcher"
  fi
  if [[ -e $installation/launcher || -L $installation/launcher ]]; then
    [[ -f $installation/launcher && ! -L $installation/launcher ]] || fail 'invalid owned launcher'
    [[ $(launcher_text) == "$(cat -- "$installation/launcher")" ]] || fail 'owned launcher was changed'
  elif [[ -e $installation/active || -L $launcher ]] && [[ ! -L $installation/removing ]]; then
    fail 'installation launcher is missing'
  fi
}

read_activation() {
  active_payload=
  previous_payload=
  local pointer=${1:-active} link receipt
  [[ -e $installation/$pointer || -L $installation/$pointer ]] || return 0
  [[ -L $installation/$pointer ]] || fail 'activation pointer is not owned'
  link=$(readlink -- "$installation/$pointer")
  [[ $link =~ ^generations/generation\.[A-Za-z0-9]+$ ]] || fail 'invalid activation pointer'
  [[ -d $installation/$link && ! -L $installation/$link ]] || fail 'activation generation is missing'
  receipt=$installation/$link/receipt.json
  [[ -f $receipt && ! -L $receipt && $(stat -c %s -- "$receipt") -le 16384 ]] || fail 'receipt is missing or invalid'
  jq -e --arg root "$installation" '
    .schema == 1 and .owner == "direct" and .installation == $root and
    (.payload | type == "string" and test("^[a-f0-9]{64}$")) and
    (.previous == null or (.previous | type == "string" and test("^[a-f0-9]{64}$")))
  ' "$receipt" >/dev/null || fail 'receipt ownership is invalid'
  active_payload=$(jq -r .payload "$receipt")
  previous_payload=$(jq -r '.previous // ""' "$receipt")
  [[ -L $installation/$link/runtime && $(readlink -- "$installation/$link/runtime") == "../../versions/$active_payload" ]] || fail 'receipt and runtime pointer disagree'
}

check_activation() {
  [[ -n $active_payload ]] || return 0
  [[ -d $installation/versions/$active_payload && ! -L $installation/versions/$active_payload ]] || fail 'active runtime is missing'
  [[ $(sha256sum -- "$installation/versions/$active_payload/payload.json") == "$active_payload "* ]] || fail 'active manifest identity differs'
  contract_compatible "$native_root/packaging/runtime.json" "$installation/versions/$active_payload/packaging/runtime.json" || fail 'active runtime dependency contract differs; the payload does not list that contract digest under upgrades'
}

lifecycle() {
  local operation=$1 result commands
  shift
  if [[ $operation == roles_disable ]]; then
    if ! commands=$("$installation/versions/$active_payload/app/launch" native roles --help 2>&1 9>&-); then
      if [[ $commands == *"error: unrecognized subcommand 'roles'"* ]]; then
        printf '%s\n' 'Skipped desktop-role reversal: this runtime has no roles entry point; no desktop role was ever enabled.'
        return
      fi
      printf '%s\n' "$commands" >&2
      fail 'native role discovery failed; runtime retained'
    fi
  fi
  result=$("$installation/versions/$active_payload/app/launch" native "$@" --json 9>&-) || fail "$operation failed; runtime retained (roles may already be disabled)"
  jq -e -s --arg operation "$operation" '
    length == 1 and (.[0] |
    .schema == 1 and .action == $operation and .error == "" and
    (if $operation == "drain" then
      (.status | IN("drained", "already_stopped")) and
      (.operation_ids | type == "array" and all(.[]; type == "string")) and
      (.dirty_note_ids | type == "array" and all(.[]; type == "string"))
    else
      .status == "complete" and .remaining_owned_entries == [] and
      (.roles | type == "object" and keys == ["autostart", "bindings", "chooser", "folder", "reveal"]) and
      all(.roles[]; (.status | IN("restored", "preserved_newer", "already_off")) and .remaining_owned_entries == [] and .error == "")
    end))
  ' <<< "$result" >/dev/null || fail "$operation returned an unknown or incomplete result; runtime retained (roles may already be disabled)"
}

activate_payload() (
  local payload=$1 previous=$2 stage generation pointer
  stage=$(mktemp -d "$installation/generations/.generation.XXXXXX")
  pointer=
  trap 'rm -rf -- "$stage"; [[ -z $pointer ]] || rm -f -- "$pointer"' EXIT
  generation=${stage##*/}
  generation=${generation#.}
  jq -n --arg root "$installation" --arg payload "$payload" --arg previous "$previous" \
    '{schema: 1, owner: "direct", installation: $root, payload: $payload, previous: (if $previous == "" then null else $previous end)}' > "$stage/receipt.json"
  chmod 600 "$stage/receipt.json"
  ln -s -- "../../versions/$payload" "$stage/runtime"
  sync -f -- "$stage/receipt.json"
  sync -f -- "$stage"
  mv -T -n -- "$stage" "$installation/generations/$generation"
  [[ ! -d $stage ]] || fail 'activation generation collision'
  sync -f -- "$installation/generations"
  pointer=$installation/.active.${generation#*.}
  ln -s -- "generations/$generation" "$pointer"
  if [[ -e $installation/removed || -L $installation/removed ]]; then
    (read_activation removed)
    rm -- "$installation/removed"
    sync -f -- "$installation"
  fi
  mv -Tf -- "$pointer" "$installation/active"
  sync -f -- "$installation"
)

install_payload() (
  local source=$1 digest destination stage
  digest=$(sha256sum -- "$source/payload.json")
  digest=${digest%% *}
  destination=$installation/versions/$digest
  if [[ -e $destination || -L $destination ]]; then
    [[ -d $destination && ! -L $destination ]] || fail 'runtime destination is not owned'
    verify_payload "$destination"
    [[ $(sha256sum -- "$destination/payload.json") == "$digest "* ]] || fail 'stored manifest identity differs'
    check_runtime "$destination"
  else
    stage=$(mktemp -d "$installation/versions/.payload.XXXXXX")
    trap 'rm -rf -- "$stage"' EXIT
    cp -a -- "$source/." "$stage/"
    chmod 755 "$stage"
    verify_payload "$stage"
    [[ $(sha256sum -- "$stage/payload.json") == "$digest "* ]] || fail 'payload changed during staging'
    check_runtime "$stage"
    sync -f -- "$stage"
    mv -T -n -- "$stage" "$destination"
    [[ ! -d $stage ]] || fail 'runtime appeared during staging'
    sync -f -- "$installation/versions"
  fi
  if [[ $digest != "$active_payload" ]]; then activate_payload "$digest" "$active_payload"; fi
  if [[ ! -L $launcher ]]; then
    ln -s -- "$installation/launcher" "$launcher"
    sync -f -- "${launcher%/*}"
  fi
  printf 'Installed %s\nLauncher: %s\nReceipt: %s\n' "$digest" "$launcher" "$installation/active/receipt.json"
)

remove_installation() {
  local version digest
  [[ -n $active_payload ]] || fail 'no owned receipt for removal'
  if [[ -L $installation/active ]]; then
    [[ ! -e $installation/discard && ! -L $installation/discard ]] || fail 'unowned removal directory'
  fi
  if [[ -e $installation/removed || -L $installation/removed ]]; then (read_activation removed); fi
  for version in "$installation/versions/"*; do
    digest=${version##*/}
    [[ $digest =~ ^[a-f0-9]{64}$ ]] || continue
    [[ -d $version && ! -L $version ]] || fail 'runtime destination is not owned'
    verify_payload "$version"
    [[ $(sha256sum -- "$version/payload.json") == "$digest "* ]] || fail 'stored manifest identity differs'
  done
  if [[ -L $installation/active ]]; then
    [[ ! -e $installation/removing && ! -L $installation/removing ]] || fail 'conflicting removal marker'
    mv -T -- "$installation/active" "$installation/removing"
    sync -f -- "$installation"
  fi
  rm -f -- "$launcher" "$installation/launcher"
  private_directory "$installation/discard"
  for version in "$installation/versions/"*; do
    digest=${version##*/}
    [[ $digest =~ ^[a-f0-9]{64}$ ]] || continue
    [[ ! -e $installation/discard/$digest && ! -L $installation/discard/$digest ]] || fail 'conflicting removal payload'
    mv -T -- "$version" "$installation/discard/$digest"
  done
  rm -rf -- "$installation/discard"
  mv -Tf -- "$installation/removing" "$installation/removed"
  sync -f -- "$installation"
  printf 'Removed direct runtime; receipts retained in %s\n' "$installation"
}

install_command() (
  local action=$1 installation launcher active_payload previous_payload source pending_launcher activation directory generations
  case $action in
    install) [[ $# == 2 ]] || fail 'usage: tools/native install PAYLOAD'; source=$(realpath -e -- "$2") ;;
    rollback|status|remove) [[ $# == 1 ]] || fail "usage: tools/native $action" ;;
  esac
  installation_paths
  command -v pacman >/dev/null || fail 'installation ownership checks require the tested Arch/Omarchy package database'
  check_owner
  if [[ $action == remove && ! -e $installation ]]; then printf 'No direct installation\n'; return; fi
  if [[ $action == install ]]; then
    verify_payload "$source"
    check_runtime "$source"
    mkdir -p -- "${installation%/*}" "${launcher%/*}"
    private_directory "$installation"
  else
    [[ -d $installation ]] || fail 'no direct installation'
    private_directory "$installation"
  fi
  [[ ! -L $installation/lock && ( ! -e $installation/lock || -f $installation/lock ) ]] || fail 'invalid installation lock'
  exec 9>"$installation/lock"
  flock -n -s 9 || fail 'another installer holds the lock'
  check_owner
  for directory in versions generations; do
    if [[ -e $installation/$directory || -L $installation/$directory ]]; then private_directory "$installation/$directory"; fi
  done
  if [[ -e $installation/removing || -L $installation/removing ]]; then
    [[ $action == remove ]] || fail 'removal interrupted; run tools/native remove again'
    [[ ! -e $installation/active && ! -L $installation/active ]] || fail 'conflicting active and removal pointers'
    read_activation removing
  elif [[ $action == remove && ! -e $installation/active && ! -L $installation/active && -L $installation/removed ]]; then
    read_activation removed
    printf 'Direct runtime already removed\n'
    return
  else
    read_activation
  fi
  activation=$(readlink -- "$installation/active" || true)
  generations=("$installation/generations/"generation.*)
  if [[ $action != status && ! -L $installation/removing ]]; then
    if [[ $action == rollback ]]; then
      [[ -n $previous_payload ]] || fail 'no previous runtime to recover'
      [[ -d $installation/versions/$previous_payload && ! -L $installation/versions/$previous_payload ]] || fail 'previous runtime is missing or not owned'
      verify_payload "$installation/versions/$previous_payload"
      [[ $(sha256sum -- "$installation/versions/$previous_payload/payload.json") == "$previous_payload "* ]] || fail 'previous manifest identity differs'
      check_runtime "$installation/versions/$previous_payload"
    fi
    if [[ -n $active_payload ]]; then
      check_activation
      verify_payload "$installation/versions/$active_payload"
      if [[ $action == remove ]]; then lifecycle roles_disable roles disable --all; fi
      lifecycle drain drain --timeout-ms 30000
    elif [[ ! -L $installation/removed && ( -e ${generations[0]} || -L ${generations[0]} ) ]]; then
      fail 'activation is missing; restore its owned receipt before lifecycle maintenance'
    fi
  fi
  if [[ $action != status ]]; then
    flock -n -x 9 || fail 'FileBlade is running or another installer holds the lock; retry after lifecycle maintenance'
    [[ $(readlink -- "$installation/active" || true) == "$activation" ]] || fail 'activation changed during lifecycle maintenance; retry'
    check_owner
    private_directory "$installation/versions"
    private_directory "$installation/generations"
  fi
  if [[ $action == status ]]; then check_activation; fi
  case $action in
    remove) remove_installation ;;
    status) [[ -n $active_payload ]] || fail 'no active installation'; cat -- "$installation/active/receipt.json" ;;
    rollback)
      activate_payload "$previous_payload" "$active_payload"
      printf 'Restored %s\n' "$previous_payload"
      ;;
    install)
      if [[ ! -f $installation/launcher ]]; then
        pending_launcher=$(mktemp "$installation/.launcher.XXXXXX")
        trap 'rm -f -- "$pending_launcher"' EXIT
        launcher_text > "$pending_launcher"
        chmod 755 "$pending_launcher"
        sync -f -- "$pending_launcher"
        mv -T -n -- "$pending_launcher" "$installation/launcher"
        [[ ! -e $pending_launcher ]] || fail 'launcher appeared during staging'
      fi
      install_payload "$source"
      ;;
  esac
)
