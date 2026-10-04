#!/usr/bin/env bash
set -euo pipefail
[[ $(hostname) == omarchy-test ]] || { printf '%s\n' 'Run only in the assigned Omarchy guest' >&2; exit 1; }
[[ $# == 2 ]] || { printf '%s\n' 'usage: 94-delivery-lifecycle.sh SOURCE PAYLOAD' >&2; exit 1; }
source_root=$(realpath -e -- "$1")
payload=$(realpath -e -- "$2")
work=$(mktemp -d /tmp/fileblade-delivery-94.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
cp -a -- "$payload" "$work/fixture"
cat > "$work/fixture/app/launch" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ $* == 'native roles --help' ]]; then printf '%s\n' 'Usage: fileblade native roles <COMMAND>'; exit 0; fi
[[ $1 == native && ${*: -1} == --json ]]
[[ ${BASH_SOURCE[0]} == "$XDG_DATA_HOME/fileblade/installation/versions/"*/app/launch ]]
flock -n -s "$XDG_DATA_HOME/fileblade/installation/lock" true
if [[ -n ${CINDER_CALLS:-} ]]; then printf '%s\n' "$2" >> "$CINDER_CALLS"; fi
if [[ $2 == roles ]]; then
  [[ $* == 'native roles disable --all --json' ]]
  if [[ ${CINDER_RESULT:-} == partial ]]; then printf '{"schema":1,"action":"roles_disable","status":"partial"}\n'; exit; fi
  jq -n '{schema:1, action:"roles_disable", status:"complete", error:"", remaining_owned_entries:[], roles:(["autostart","bindings","chooser","folder","reveal"] | map({key:.,value:{status:"already_off",remaining_owned_entries:[],error:""}}) | from_entries)}'
else
  [[ $* == 'native drain --timeout-ms 30000 --json' ]]
  if [[ -n ${CINDER_ACTIVE:-} ]]; then
    readlink "$CINDER_ACTIVE" > "$CINDER_ACTIVE.saved"
    ln -sfn generations/generation.changed "$CINDER_ACTIVE"
  fi
  if [[ ${CINDER_RESULT:-} == failure ]]; then exit 3; fi
  if [[ ${CINDER_RESULT:-} == answered ]]; then
    printf '%s\n' '{"schema":1,"action":"drain","status":"busy","operation_ids":[],"dirty_note_ids":[],"error":"layout is not writable"}'
    exit 3
  fi
  status=${CINDER_RESULT:-drained}
  if [[ $status == noisy ]]; then
    printf '%s\n' 'fileblade: startup recovery finished'
    status=drained
  fi
  jq -n --arg status "$status" '{schema:1,action:"drain",status:$status,operation_ids:[],dirty_note_ids:[],error:""}'
fi
EOF
digest=$(sha256sum "$work/fixture/app/launch")
jq --arg digest "${digest%% *}" '.files |= map(if .path == "app/launch" then .sha256 = $digest else . end)' "$payload/payload.json" > "$work/fixture/payload.json"
native=$work/fixture/tools/native
export HOME=$work/home XDG_DATA_HOME=$work/data XDG_CONFIG_HOME=$work/config XDG_STATE_HOME=$work/state
mkdir -p "$HOME"
"$native" install "$work/fixture"
installation=$XDG_DATA_HOME/fileblade/installation
cp "$installation/active/receipt.json" "$work/receipt"
export CINDER_CALLS=$work/calls
for result in partial busy unexpected failure; do
  if CINDER_RESULT=$result "$native" remove > "$work/refusal" 2>&1; then exit 1; fi
  cmp "$work/receipt" "$installation/active/receipt.json"
  [[ -L $HOME/.local/bin/fileblade && ! -L $installation/removing ]]
done
printf 'PASS E-94-01 unsuccessful or unknown drain preserves active runtime\n'
if CINDER_ACTIVE=$installation/active "$native" install "$work/fixture" > "$work/refusal" 2>&1; then exit 1; fi
grep -F 'activation changed during lifecycle maintenance' "$work/refusal"
ln -sfn "$(cat "$installation/active.saved")" "$installation/active"
cmp "$work/receipt" "$installation/active/receipt.json"
printf 'PASS E-94-02 changed activation identity refuses after drain\n'
: > "$CINDER_CALLS"
"$native" remove
[[ $(cat "$CINDER_CALLS") == $'roles\ndrain' ]]
printf 'PASS E-94-03 removal reverses roles before drain\n'
"$native" install "$work/fixture" > /dev/null
cp "$installation/active/receipt.json" "$work/receipt"
if CINDER_RESULT=answered "$native" install "$work/fixture" > "$work/refusal" 2>&1; then exit 1; fi
grep -F 'drain failed (status busy: layout is not writable); runtime retained' "$work/refusal"
if grep -F 'parse error' "$work/refusal"; then exit 1; fi
cmp "$work/receipt" "$installation/active/receipt.json"
printf 'PASS E-94-04 a refused drain reports its status and error\n'
CINDER_RESULT=noisy "$native" install "$work/fixture" > "$work/noisy" 2>&1
if grep -F 'parse error' "$work/noisy"; then exit 1; fi
grep -F 'Installed ' "$work/noisy"
printf 'PASS E-94-05 a notice before the drain answer is tolerated\n'
"$native" remove
unset CINDER_CALLS
"$source_root/tests/vm/expectations/91-delivery-install.sh" "$work/fixture"
"$source_root/tests/vm/expectations/93-delivery-remove.sh" "$work/fixture"
