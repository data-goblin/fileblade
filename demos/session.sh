#!/usr/bin/env bash
set -euo pipefail
umask 077

DEMOCTL_REPO=${DEMOCTL_REPO:-$HOME/git/omarchy-demo}
SESSION=$DEMOCTL_REPO/scripts/demo-session
BASE=${FILEBLADE_DEMO_BASE:-${XDG_RUNTIME_DIR:+$XDG_RUNTIME_DIR/fileblade-demo}}
REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
OUT_DIR=${FILEBLADE_DEMO_OUT:-$REPO/assets/demos}

die() { echo "session: $*" >&2; exit 1; }

validate_base() {
  [[ -n $BASE ]] || die "XDG_RUNTIME_DIR is unset; set FILEBLADE_DEMO_BASE to a dedicated absolute directory"
  [[ $BASE == /* ]] || die "demo base must be an absolute path: $BASE"
  case "${BASE%/}" in
  "" | / | "$HOME" | "${XDG_CONFIG_HOME:-$HOME/.config}" | "${XDG_STATE_HOME:-$HOME/.local/state}" | "${XDG_DATA_HOME:-$HOME/.local/share}")
    die "refusing unsafe demo base: $BASE"
    ;;
  esac
  [[ ! -L $BASE ]] || die "refusing symlink demo base: $BASE"
  mkdir -p -m 700 -- "$BASE"
  [[ -d $BASE && ! -L $BASE ]] || die "demo base is not a directory: $BASE"
  [[ $(stat -c %u -- "$BASE") == $(id -u) ]] || die "demo base is not owned by the current user: $BASE"
  chmod 700 -- "$BASE"
}

validate_base
FAKE_HOME=$BASE/home
SHELL_ROOT=$BASE/omarchy
STATE_HOME=$BASE/state
SHIM=$BASE/shim
PIDFILE=$BASE/quickshell.pid
TERMINAL_PROGRAM=${FILEBLADE_DEMO_TERMINAL:-shell}

stop_nested_herdr() {
  [[ -S $FAKE_HOME/.config/herdr/herdr.sock ]] || return 0
  env -u HERDR_SESSION -u HERDR_SOCKET -u HERDR_SOCKET_PATH HOME=$FAKE_HOME XDG_CONFIG_HOME=$FAKE_HOME/.config \
    herdr server stop >/dev/null 2>&1 || true
}

build_home() {
  stop_nested_herdr
  rm -rf "$FAKE_HOME" "$STATE_HOME"
  mkdir -p "$FAKE_HOME/.config" "$FAKE_HOME/.local/share/zoxide" "$FAKE_HOME/.local/state" "$STATE_HOME"
  cp -a --no-preserve=links "$HOME/.config/omarchy" "$FAKE_HOME/.config/omarchy"
  rm -rf "$FAKE_HOME/.config/omarchy/plugins" "$FAKE_HOME/.config/omarchy/fileblade"
  mkdir -p "$FAKE_HOME/.config/omarchy/plugins" "$FAKE_HOME/.config/omarchy/fileblade"
  local plugin
  for plugin in "$HOME"/.config/omarchy/plugins/*; do
    case "$(basename "$plugin")" in kurt.notifications|kurt.agents|omarchy.notifications) continue ;; esac
    ln -s "$(readlink -f "$plugin")" "$FAKE_HOME/.config/omarchy/plugins/$(basename "$plugin")"
  done
  build_agent_home
  for cfg in hypr alacritty ghostty kitty nvim; do
    [[ -e $HOME/.config/$cfg ]] && ln -s "$(readlink -f "$HOME/.config/$cfg")" "$FAKE_HOME/.config/$cfg"
  done
  ln -s "$HOME/.local/share/fonts" "$FAKE_HOME/.local/share/fonts" 2>/dev/null || true
  ln -s "$HOME/.local/share/nvim" "$FAKE_HOME/.local/share/nvim" 2>/dev/null || true
  ln -s "$HOME/.local/state/nvim" "$FAKE_HOME/.local/state/nvim" 2>/dev/null || true
  mkdir -p "$FAKE_HOME/.local/state/omarchy"
  cp -a --no-preserve=links "$HOME/.local/state/omarchy/current" "$FAKE_HOME/.local/state/omarchy/current" 2>/dev/null || true
  ln -s "$HOME/.local/share/icons" "$FAKE_HOME/.local/share/icons" 2>/dev/null || true
  ln -s "$HOME/.local/share/applications" "$FAKE_HOME/.local/share/applications" 2>/dev/null || true

  local demo=$FAKE_HOME/Projects
  mkdir -p "$demo/sunrise-site/src/components" "$demo/sunrise-site/docs" "$demo/sunrise-site/assets" \
           "$demo/notes" "$demo/recipes" "$FAKE_HOME/Downloads" "$FAKE_HOME/Pictures" "$FAKE_HOME/Documents"
  cat > "$demo/sunrise-site/README.md" <<'EOF'
# Sunrise

A tiny static site about mornings. Build it with `make`, serve it with anything.
EOF
  printf 'export default function Header() {\n  return <header>Sunrise</header>\n}\n' > "$demo/sunrise-site/src/components/Header.tsx"
  printf 'export default function Footer() {\n  return <footer>Made before coffee</footer>\n}\n' > "$demo/sunrise-site/src/components/Footer.tsx"
  printf 'import Header from "./components/Header"\n\nexport const app = () => Header\n' > "$demo/sunrise-site/src/index.ts"
  printf '# Getting started\n\nClone, build, enjoy the sunrise.\n' > "$demo/sunrise-site/docs/getting-started.md"
  printf '# Deploying\n\nCopy `dist/` anywhere that serves files.\n' > "$demo/sunrise-site/docs/deploying.md"
  printf 'all:\n\tmkdir -p dist && cp -r src docs dist/\n' > "$demo/sunrise-site/Makefile"
  printf '{ "name": "sunrise", "version": "0.3.0" }\n' > "$demo/sunrise-site/package.json"
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i "gradients=s=1280x720:c0=#f7a76c:c1=#7a3b8f:c2=#2b1d4e:n=3:d=1" -frames:v 1 "$demo/sunrise-site/assets/hero.png"
  printf 'Groceries\n- oats\n- oranges\n- coffee, the good one\n' > "$demo/notes/groceries.md"
  printf 'Ideas\n- a blade for bookmarks\n- a blade for the clipboard\n' > "$demo/notes/ideas.md"
  printf 'Pancakes\n\nFlour, eggs, milk, a pinch of salt. Rest the batter.\n' > "$demo/recipes/pancakes.md"
  printf 'Shakshuka\n\nTomatoes, peppers, eggs, cumin. Bread on the side.\n' > "$demo/recipes/shakshuka.md"
  printf 'quarterly-report.pdf placeholder\n' > "$FAKE_HOME/Downloads/quarterly-report.pdf"
  printf 'invoice.pdf placeholder\n' > "$FAKE_HOME/Documents/invoice-2026-08.pdf"
  (
    cd "$demo/sunrise-site"
    git init -q -b main
    git -c user.name=Demo -c user.email=demo@example.com add -A
    git -c user.name=Demo -c user.email=demo@example.com commit -q -m "Initial sunrise"
    printf '\nRun `make` and open `dist/index.html`.\n' >> README.md
    printf 'export const version = "0.3.1"\n' > src/version.ts
  )
  local dir
  for dir in "$demo/sunrise-site" "$demo/notes" "$demo/recipes" "$FAKE_HOME/Downloads" "$FAKE_HOME/Documents" "$demo/sunrise-site/docs"; do
    HOME=$FAKE_HOME _ZO_DATA_DIR=$FAKE_HOME/.local/share/zoxide zoxide add "$dir"
  done
  cat > "$FAKE_HOME/.config/omarchy/fileblade/blades.json" <<EOF
{"version":1,"monitorMode":"all","animations":true,"blades":{
 "left":{"open":false,"width":500,"mode":"docked","slots":[
   {"id":"files","modules":[{"module":"files","state":{"root":"$demo/sunrise-site"}}],"active":0,"collapsed":false,"fraction":0.58},
   {"id":"properties","modules":[{"module":"properties","state":{}}],"active":0,"collapsed":false,"fraction":0.42}]},
 "right":{"open":false,"width":500,"mode":"docked","slots":[
   {"id":"skills","modules":[{"module":"data-goblin.fileblade-skills/skills","state":{"metric":"tokens","sort":[{"key":"tokens","desc":true}],"columns":["tokens"]}},{"module":"data-goblin.fileblade-hooks/hooks","state":{}},{"module":"data-goblin.fileblade-mcp/mcp","state":{}}],"active":0,"collapsed":false,"fraction":0.55},
   {"id":"memory","modules":[{"module":"data-goblin.fileblade-memory/memory","state":{"columns":["tokens"],"metric":"tokens"}},{"module":"notes","state":{"text":{"version":2,"revision":1,"activeId":"note-1","nextId":2,"items":[{"id":"note-1","label":"Today","text":"- [x] ~~Record the demos~~\n- [ ] Write the README\n- [ ] Share it"}]}}}],"active":0,"collapsed":false,"fraction":0.45}]}}}
EOF
}

build_agent_home() {
  local c=$FAKE_HOME/.claude
  mkdir -p "$c/skills/docx" "$c/skills/release-notes" "$c/skills/pdf-tools" "$c/rules" "$FAKE_HOME/.codex" "$FAKE_HOME/.agents/skills"
  printf -- '---\nname: docx\ndescription: Create and edit Word documents with tracked changes, tables of contents and templates.\n---\n\n# docx\n\nUse python-docx for edits and LibreOffice headless for PDF export. Keep styles from the template.\n' > "$c/skills/docx/SKILL.md"
  printf -- '---\nname: release-notes\ndescription: Turn merged pull requests since the last tag into release notes grouped by area.\n---\n\n# release-notes\n\nList PRs with gh, group by label, write one line each, link the PR. Breaking changes first.\n' > "$c/skills/release-notes/SKILL.md"
  printf -- '---\nname: pdf-tools\ndescription: Merge, split, rotate and OCR PDF files from the command line.\n---\n\n# pdf-tools\n\nqpdf for structure, ocrmypdf for text layers. Never rasterise a PDF that already has text.\n' > "$c/skills/pdf-tools/SKILL.md"
  printf -- '# Code\n\n- Prefer small pull requests\n- Write the test first when fixing a bug\n- No commented-out code\n' > "$c/rules/CODE.md"
  printf -- '# Git\n\n- Commit prefixes: Feat, Fix, Docs, Clean\n- Rebase before pushing\n- Delete merged branches\n' > "$c/rules/GIT.md"
  printf -- '# Writing\n\n- Short sentences\n- Numbers over adjectives\n- Say who did what\n' > "$c/rules/WRITING.md"
  printf -- '# Project notes\n\nSunrise is a static site. Build with make, deploy by copying dist/.\n' > "$c/CLAUDE.md"
  cat > "$c/settings.json" <<'JSON'
{
  "hooks": {
    "PostToolUse": [
      { "matcher": "Edit|Write", "hooks": [ { "type": "command", "command": "prettier --write \"$FILE\"" } ] }
    ],
    "Notification": [
      { "hooks": [ { "type": "command", "command": "notify-send 'Claude' 'needs you'" } ] }
    ]
  }
}
JSON
  printf -- '[mcp_servers.docs]\ncommand = "npx"\nargs = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp/demo/home/Projects"]\n' > "$FAKE_HOME/.codex/config.toml"
  printf -- '# Agents\n\nShared instructions for every agent on this machine.\n' > "$FAKE_HOME/.codex/AGENTS.md"
  ln -s "$c/skills/release-notes" "$FAKE_HOME/.agents/skills/release-notes"
}

build_shell() {
  rm -rf "$SHELL_ROOT" "$SHIM"
  mkdir -p "$SHELL_ROOT" "$SHIM"
  cp -r /usr/share/omarchy/shell "$SHELL_ROOT/shell"
  local entry
  for entry in /usr/share/omarchy/*; do
    [[ $(basename "$entry") == shell ]] && continue
    ln -s "$entry" "$SHELL_ROOT/$(basename "$entry")"
  done
  cat > "$SHIM/hyprctl" <<'EOF'
#!/usr/bin/env bash
if [[ ${1:-} == eval && ${2:-} == *no_hardware_cursors* ]]; then echo ok; exit 0; fi
exec /usr/bin/hyprctl "$@"
EOF
  chmod +x "$SHIM/hyprctl"
}

session_vars() {
  local name
  for name in $(compgen -e | grep '^HERDR_' || true); do unset "$name"; done
  eval "$("$SESSION" env)"
  export WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE
  export OMARCHY_PATH=$SHELL_ROOT
  export HOME=$FAKE_HOME
  export XDG_CONFIG_HOME=$FAKE_HOME/.config
  export XDG_STATE_HOME=$STATE_HOME
  export XDG_CACHE_HOME=$BASE/cache
  export XDG_DATA_HOME=$FAKE_HOME/.local/share
  export _ZO_DATA_DIR=$FAKE_HOME/.local/share/zoxide
  export PATH=$SHIM:$PATH
  export FILEBLADE_BINARY=$REPO/target/release/fileblade
}

up() {
  local size=${1:-1920x1080}
  mkdir -p "$OUT_DIR"
  build_home
  build_shell
  "$SESSION" start "$size"
  session_vars
  env -u HERDR_SESSION -u HERDR_SOCKET QS_DISABLE_FILE_WATCHER=1 QS_NO_RELOAD_POPUP=1 \
    setsid quickshell -n -p "$SHELL_ROOT/shell" > "$BASE/quickshell.log" 2>&1 &
  echo $! > "$PIDFILE"
  local i ok=""
  for i in $(seq 1 60); do
    sleep 0.5
    if omarchy-shell shell ping >/dev/null 2>&1; then ok=1; break; fi
  done
  [[ -n $ok ]] || die "nested shell did not answer ping; see $BASE/quickshell.log"
  sleep 2
  if [[ $TERMINAL_PROGRAM == herdr ]]; then
    (cd "$FAKE_HOME/Projects/sunrise-site" && env -u HERDR_SESSION -u HERDR_SOCKET -u HERDR_SOCKET_PATH \
      setsid foot -o font=monospace:size=14 -e herdr >/dev/null 2>&1 &)
    sleep 3
  else
    setsid foot -o font=monospace:size=14 -e bash -c "cd $FAKE_HOME/Projects/sunrise-site; export PS1='\\[\\e[1;35m\\]sunrise-site\\[\\e[0m\\] \\$ '; clear; ls; exec bash --norc -i" >/dev/null 2>&1 &
    sleep 1.5
  fi
  /usr/bin/hyprctl dismissnotify >/dev/null 2>&1 || true
  echo "session up on $WAYLAND_DISPLAY; shell at $SHELL_ROOT; home $FAKE_HOME"
  fileblade status 2>/dev/null | jq -c '{open, focusedBlade}' || true
}

down() {
  stop_nested_herdr
  if [[ -f $PIDFILE ]]; then
    local pid; pid=$(cat "$PIDFILE")
    kill "$pid" 2>/dev/null || true
    pkill -P "$pid" 2>/dev/null || true
    rm -f "$PIDFILE"
  fi
  "$SESSION" stop || true
}

reset_state() {
  session_vars
  fileblade blade dock left >/dev/null 2>&1 || true
  fileblade blade dock right >/dev/null 2>&1 || true
  fileblade blade set left files,properties >/dev/null 2>&1 || true
  fileblade root "$FAKE_HOME/Projects/sunrise-site" >/dev/null 2>&1 || true
  fileblade clear-search >/dev/null 2>&1 || true
  fileblade search-deep false >/dev/null 2>&1 || true
  local sub
  for sub in assets docs src src/components; do
    fileblade collapse "$FAKE_HOME/Projects/sunrise-site/$sub" >/dev/null 2>&1 || true
  done
  fileblade blade close left >/dev/null 2>&1 || true
  fileblade blade close right >/dev/null 2>&1 || true
  sleep 1.2
  fileblade status 2>/dev/null | jq -c '{open, focusedBlade, root: .rootPath}' || true
}

case "${1:-}" in
up) shift; up "$@" ;;
reset) reset_state ;;
down) down ;;
env) session_vars; env | grep -E '^(WAYLAND_DISPLAY|HYPRLAND_INSTANCE_SIGNATURE|OMARCHY_PATH|HOME|XDG_CONFIG_HOME|XDG_STATE_HOME|XDG_CACHE_HOME|PATH|FILEBLADE_BINARY)=' | sed 's/^/export /' ;;
exec) shift; session_vars; exec "$@" ;;
shot) shift; session_vars; grim -o WAYLAND-1 "${1:-shot}.png" && echo "${1:-shot}.png" ;;
record|render|run) cmd=$1; shift; session_vars; exec democtl "$cmd" "$@" --out "$OUT_DIR" ;;
*) printf '%s\n' \
  'Usage: demos/session.sh up [WxH] | reset | down | env | exec CMD... | shot NAME' \
  '       demos/session.sh {record|render|run} X.toml' \
  'The default resolution is 1920x1080; FILEBLADE_DEMO_TERMINAL=herdr selects herdr.'; exit 1 ;;
esac
