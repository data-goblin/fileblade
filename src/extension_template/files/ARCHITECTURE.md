# {{PLUGIN_NAME}} architecture

FileBlade (`data-goblin.fileblade`) hosts this extension. The Omarchy shell
loads `Service.qml` once per enabled plugin; FileBlade instantiates
`blades/Module.qml` once per slot, and potentially once per screen, that shows
it. Nothing here runs unless FileBlade is installed and enabled.

This file was written by an agent.

Host detection checks the installed native FileBlade launcher and its view before
falling back to legacy Omarchy plugin detection. A healthy native installation
does not require the old host plugin to appear in the Omarchy catalogue.
The Enable action is reserved for an explicitly disabled legacy host plugin.

## Ownership

```yaml
Service.qml:         the singleton provider. Shared scanners, caches, watchers and mutations belong here, with one teardown when the plugin is disabled or the shell reloads
blades/Module.qml:   visual and lightweight. Binds to context.providerService, renders plain text, routes keys, keeps per-tab presentation state in context.state
HostGuard.qml, .js:  shown while FileBlade is missing or disabled; explains manual installation without downloading anything, or enables an installed host and restarts the shell
manifest.json:       the module definition (id, entry, hostContract, settings schema); FileBlade reads it from the plugin registry
scripts/:            fileblade-extension-image.py regenerates assets/fileblade-extension-logo.svg
tests/:              tests/run is the local gate; tests/imports stubs qs.Commons so the module loads offline
```

## Where state lives

- `context.state` is per tab and saved with the blade layout; keep it small
- `context.stateDir` and `context.configDir` are
  `~/.local/state/omarchy/fileblade/modules/{{PLUGIN_ID}}+{{MODULE_ID}}/` and
  `~/.config/omarchy/fileblade/config/{{PLUGIN_ID}}+{{MODULE_ID}}/`, created
  `0700` by the FileBlade backend when the module loads
- settings declared in the manifest schema are ordinary `context.state` keys
  read through `context.settings.get(key)`; blade settings renders the form

## What the host does not do

FileBlade validates the definition and namespaces the module id; it does not
sandbox the QML. Render filesystem values as plain text, bound every model, do
not start a `Process` of your own, and leave FileBlade through the shared
primitives (`openDefault`, `openInEditor`, `revealInFileManager`) so focus
hand-off stays consistent. FileBlade's SECURITY.md carries the review checklist.
