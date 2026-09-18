This file was written by an agent.

# FileBlade {{PLUGIN_NAME}}

An Omarchy Quattro plugin that adds the `{{MODULE_ID}}` blade module to
FileBlade. {{DESCRIPTION}}

## Required dependency

**FileBlade (manifest id `data-goblin.fileblade`) must be installed and
enabled before this plugin does anything.** The plugin ships no window, bar,
panel or menu of its own. It contributes one module to the
`data-goblin.fileblade/blade` socket that FileBlade hosts.

Without FileBlade:

- the manifest's `extensions` block is inert JSON and nothing loads it
- the service starts and does nothing
- the host guard offers to enable FileBlade when it is installed but off, and
  restarts the shell; it never installs anything

The module declares `hostContract: 2`. FileBlade lists but refuses a module
whose contract is newer than its own.

### Install order

```bash
omarchy plugin add https://github.com/data-goblin/fileblade.git --enable
omarchy plugin add {{REPOSITORY}} --yes --enable
omarchy restart shell
fileblade blade add right {{PLUGIN_ID}}/{{MODULE_ID}}
```

### Removal

```bash
omarchy plugin remove {{PLUGIN_ID}}
omarchy restart shell
```

## Layout

```yaml
manifest.json:                         plugin identity and the blade module definition
Service.qml:                           singleton provider; shared work and the host guard loader
HostGuard.qml, HostGuard.js:           the missing-host guard
blades/Module.qml:                     the blade module
assets/:                               fileblade-extension-logo.svg (README banner), fileblade-logo.png (host guard)
tests/run:                             local gate
tests/imports/qs/Commons/:             offline stub of the shell's Style, Color and Util singletons
docs/agent-guidelines.md:              rules for coding agents working in this extension
docs/agent-written/README.md:          this guide
ARCHITECTURE.md:                       ownership and state
```

## Settings

```yaml
caption:        string, up to 64 characters, shown under the selection; empty shows the key hint
showSelection:  boolean, show the selected path's name
```

Both rows render in the blade settings sheet from the manifest schema and are
read with `context.settings.get(key)`.

## Keys

```yaml
Enter:            open the selected path with its default application (folders navigate the Files blade)
e:                open the selected path in the editor
Tab, Shift+Tab:   next and previous slot
Esc:              close the blade
?:                FileBlade's shortcut overlay, fed by the module's shortcuts property
```

## Developing

1. Link the checkout: `ln -s "$PWD" ~/.config/omarchy/plugins/{{PLUGIN_ID}}`,
   then `omarchy plugin enable {{PLUGIN_ID}}` and `omarchy restart shell`
2. QML does not hot-reload; run `omarchy restart shell` after each edit. If
   Quickshell reports an error at an impossible line, move
   `~/.cache/quickshell/qmlcache` aside
3. `fileblade modules` lists the module; `fileblade blade add right
   {{PLUGIN_ID}}/{{MODULE_ID}}` places it
4. After any QML change, check `journalctl --user -b` for "plugin load failed"
5. `tests/run` before committing; `omarchy plugin validate .` before publishing
6. After a rename, regenerate the banner:
   `fileblade extension image --png`

## Going further

- Inventory-style blades load `context.ui.url("ArtifactTree")` and the shared
  `ArtifactInventory` runtime; see "Satellites and artifact bins" in
  FileBlade's EXTENSIONS.md
- Script actions in the file menu are `data-goblin.fileblade/action` entries in
  the manifest and need no QML
- Bounded helpers (`data-goblin.fileblade/helper`) run a bundled executable
  through FileBlade's resident backend with capped input, output and time
