This file was written by an agent.

# FileBlade {{PLUGIN_NAME}}

A native FileBlade extension providing the `{{MODULE_ID}}` blade module.
{{DESCRIPTION}}

## Installation

Install and start FileBlade, then register this trusted checkout:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/fileblade/extensions"
git clone {{REPOSITORY}} "${XDG_CONFIG_HOME:-$HOME/.config}/fileblade/extensions/{{PLUGIN_ID}}"
fileblade rescan-modules
fileblade blade add right {{PLUGIN_ID}}/{{MODULE_ID}}
```

The module declares `hostContract: 2`. FileBlade lists but refuses a module
whose contract is newer than its own. Review the extension before registration:
its QML executes with your desktop user's authority.

To remove it, close its tabs, remove its registration from the native extension
directory and run `fileblade rescan-modules`. When the registration is a symlink,
remove that link to preserve the source checkout.

## Layout

```yaml
manifest.json:                         extension identity and the blade module definition
Provider.qml:                          shared provider runtime owned by FileBlade
Service.qml:                           compatibility wrapper and host guard loader
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

1. Link a trusted checkout under the native extension directory and run
   `fileblade rescan-modules`.
2. Use `fileblade modules` to confirm discovery and `fileblade blade add right
   {{PLUGIN_ID}}/{{MODULE_ID}}` to place it.
3. Work in an isolated desktop. After changing QML, drain the native runtime
   with `fileblade native drain` and start `fileblade` again.
4. Run `tests/run` and `fileblade extension check .` before committing.
5. After a rename, regenerate the banner with `fileblade extension image --png`.

## Going further

- Inventory-style blades load `context.ui.url("ArtifactTree")` and the shared
  `ArtifactInventory` runtime; see "Satellites and artifact bins" in
  FileBlade's EXTENSIONS.md
- Script actions in the file menu are `data-goblin.fileblade/action` entries in
  the manifest and need no QML
- Bounded helpers (`data-goblin.fileblade/helper`) run a bundled executable
  through FileBlade's resident backend with capped input, output and time
