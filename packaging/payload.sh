target_machine() {
  case $1 in
    x86_64-unknown-linux-gnu|x86_64-unknown-linux-musl) printf '%s\n' 'Advanced Micro Devices X86-64' ;;
    *) fail "unsupported Linux target: $1" ;;
  esac
}

check_elf() {
  local binary=$1 target=$2 header program dynamic interpreter
  header=$(readelf -h -- "$binary") || fail 'backend is not ELF'
  [[ $header == *ELF64* && $header == *'little endian'* && $header == *"$(target_machine "$target")"* ]] || fail "backend architecture differs from $target"
  program=$(readelf -l -- "$binary")
  dynamic=$(readelf -d -- "$binary")
  if [[ $target == *-musl ]]; then
    [[ $program != *INTERP* && $dynamic != *NEEDED* ]] || fail 'musl delivery requires a static backend'
  else
    interpreter=/lib64/ld-linux-x86-64.so.2
    [[ $program == *"Requesting program interpreter: $interpreter]"* ]] || fail 'backend GNU ABI interpreter differs'
  fi
}

payload_inventory() {
  local root=$1 entry mode digest
  (cd -- "$root" && find . -mindepth 1 -print0) | sort -z | while IFS= read -r -d '' entry; do
    entry=${entry#./}
    [[ $entry != payload.json ]] || continue
    [[ $entry =~ ^[A-Za-z0-9_.+/-]+$ && $entry != /* && $entry != *'//'* && /$entry/ != *'/../'* && /$entry/ != *'/./'* ]] || fail "unsupported payload path: $entry"
    [[ ! -L $root/$entry ]] || fail "payload symlink: $entry"
    if [[ -d $root/$entry ]]; then
      [[ $(stat -c %a -- "$root/$entry") == 755 ]] || fail "invalid directory mode: $entry"
      continue
    fi
    [[ -f $root/$entry ]] || fail "special payload file: $entry"
    mode=$(stat -c %a -- "$root/$entry")
    [[ $mode == 644 || $mode == 755 ]] || fail "invalid payload mode: $entry"
    digest=$(sha256sum -- "$root/$entry")
    printf '%s\t%s\t%s\n' "$entry" "$mode" "${digest%% *}"
  done
}

dependency_contract() {
  jq -cS '{schema, backend, commands, packages}' "$1"
}

contract_digest() {
  dependency_contract "$1" | sha256sum | cut -d ' ' -f 1
}

contract_compatible() {
  [[ $(dependency_contract "$1") == "$(dependency_contract "$2")" ]] && return 0
  jq -e --arg digest "$(contract_digest "$2")" '(.upgrades // []) | index($digest) != null' "$1" >/dev/null
}

verify_payload() {
  local root manifest actual expected target required
  root=$(realpath -e -- "$1")
  manifest=$root/payload.json
  [[ -f $manifest && ! -L $manifest && $(stat -c %s -- "$manifest") -le 4194304 ]] || fail 'missing or oversized payload manifest'
  jq -e '
    .schema == 1 and .kind == "fileblade-native" and
    (.version | type == "string" and test("^[0-9]+\\.[0-9]+\\.[0-9]+([+-][A-Za-z0-9.-]+)?$")) and
    (.source | type == "string" and test("^([a-f0-9]{40}|[a-f0-9]{64})$")) and
    (.target | IN("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl")) and
    (.architecture == (.target | split("-")[0])) and
    (.files | type == "array" and length > 0 and length <= 10000) and
    (all(.files[]; (.path | type == "string" and test("^[A-Za-z0-9_.+/-]+$")) and
      (.mode | IN("644", "755")) and (.sha256 | type == "string" and test("^[a-f0-9]{64}$")))) and
    ([.files[].path] == ([.files[].path] | sort | unique))
  ' "$manifest" >/dev/null || fail 'invalid payload manifest'
  actual=$(payload_inventory "$root") || fail 'invalid payload tree'
  expected=$(jq -r '.files[] | [.path, .mode, .sha256] | @tsv' "$manifest")
  [[ $actual == "$expected" ]] || fail 'payload inventory, mode or digest differs'
  while IFS= read -r required; do
    [[ -f $root/$required ]] || fail "missing runtime file: $required"
  done < <(jq -r '.required[]' "$native_root/packaging/runtime.json")
  [[ -x $root/app/launch && -x $root/bin/fileblade && -x $root/tools/native ]] || fail 'runtime entrypoint is not executable'
  [[ $(jq -r .version "$root/manifest.json") == "$(jq -r .version "$manifest")" ]] || fail 'runtime version differs'
  contract_compatible "$native_root/packaging/runtime.json" "$root/packaging/runtime.json" || fail 'payload dependency contract differs from installer'
  target=$(jq -r .target "$manifest")
  check_elf "$root/bin/fileblade" "$target"
  printf 'Verified FileBlade %s (%s)\n' "$(jq -r .version "$manifest")" "$target"
}

stage_payload() (
  local source binary target=$3 notices output stage path relative mode version source_id filelist
  source=$(realpath -e -- "$1")
  binary=$(realpath -e -- "$2")
  notices=$(realpath -e -- "$4")
  output=$(realpath -m -- "$5")
  [[ ! -e $output && ! -L $output && -d ${output%/*} ]] || fail 'output must be absent with an existing parent'
  [[ -x $binary && -f $notices && ! -L $binary ]] || fail 'backend or target notices unavailable'
  target_machine "$target" >/dev/null
  check_elf "$binary" "$target"
  source_id=$(git -C "$source" rev-parse HEAD)
  git -C "$source" diff --quiet HEAD -- || fail 'source has tracked changes; commit the candidate before staging'
  stage=$(mktemp -d "${output%/*}/.fileblade-payload.XXXXXX")
  trap 'rm -rf -- "$stage"' EXIT
  chmod 755 "$stage"
  filelist=$(git -C "$source" ls-files -- $(jq -r '.roots[]' "$native_root/packaging/runtime.json"))
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    case $path in app/ovm-spike|app/qualification/*|app/ADAPTER.md) continue ;; esac
    [[ $path =~ ^[A-Za-z0-9_.+/-]+$ && -f $source/$path && ! -L $source/$path ]] || fail "unsupported runtime source: $path"
    mode=644
    [[ ! -x $source/$path ]] || mode=755
    install -D -m "$mode" -- "$source/$path" "$stage/$path"
  done <<< "$filelist"
  install -D -m 755 -- "$binary" "$stage/bin/fileblade"
  install -m 644 -- "$notices" "$stage/THIRD_PARTY_NOTICES.html"
  install -D -m 755 -- "$native_tool" "$stage/tools/native"
  for path in runtime.json payload.sh install.sh; do
    install -D -m 644 -- "$native_root/packaging/$path" "$stage/packaging/$path"
  done
  version=$(jq -er .version "$stage/manifest.json")
  payload_inventory "$stage" | jq -Rn --arg version "$version" --arg target "$target" --arg source "$source_id" '
    {schema: 1, kind: "fileblade-native", version: $version, target: $target,
     architecture: ($target | split("-")[0]), source: $source,
     files: [inputs | split("\t") | {path: .[0], mode: .[1], sha256: .[2]}]}
  ' > "$stage/payload.json"
  chmod 644 "$stage/payload.json"
  verify_payload "$stage"
  git -C "$source" diff --quiet HEAD -- || fail 'source changed during staging'
  [[ $(git -C "$source" rev-parse HEAD) == "$source_id" ]] || fail 'source commit changed during staging'
  mv -T -n -- "$stage" "$output"
  [[ ! -d $stage ]] || fail 'output appeared during staging'
  printf '%s\n' "$output"
)

check_runtime() {
  local root=$1 target architecture dependency program output
  local -a dependencies
  target=$(jq -r .target "$root/payload.json")
  architecture=${target%%-*}
  [[ $(uname -m) == "$architecture" ]] || fail "payload requires $architecture, machine is $(uname -m)"
  while IFS= read -r dependency; do
    command -v "$dependency" >/dev/null || fail "required command unavailable: $dependency"
  done < <(jq -r '.commands[]' "$native_root/packaging/runtime.json")
  command -v pacman >/dev/null || fail 'dependency qualification currently requires the tested Arch/Omarchy package database'
  mapfile -t dependencies < <(jq -r '.packages[]' "$native_root/packaging/runtime.json")
  output=$(pacman -T "${dependencies[@]}") || fail "missing or incompatible runtime packages: $output"
  program=$root/bin/fileblade
  if [[ $target == *-gnu ]]; then
    output=$(timeout 10 ldd -- "$program") || fail 'backend ABI dependencies cannot be resolved'
    [[ $output != *'not found'* ]] || fail "backend ABI dependency unavailable: $output"
  fi
  output=$(timeout 10 "$program" --version) || fail 'backend cannot run on this ABI'
  [[ $output == "fileblade $(jq -r .version "$root/payload.json")" ]] || fail 'backend version differs from payload'
  pacman -Q quickshell qt6-base qt6-declarative glib2 xdg-desktop-portal
}
