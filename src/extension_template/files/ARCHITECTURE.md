This file was written by an agent.

# {{PLUGIN_NAME}} architecture

The native FileBlade app hosts this extension. Registration under
`~/.config/fileblade/extensions/{{PLUGIN_ID}}` makes its manifest discoverable.
FileBlade owns one `Provider.qml` runtime and loads `blades/Module.qml` for each
slot and screen that displays it.

## Ownership

```yaml
Provider.qml:       shared scanners, caches, watchers and mutations; attachment starts work and shutdown releases it
blades/Module.qml:  presentation, plain-text rendering, shared focus commands and per-tab context.state
manifest.json:      module identity, entry, provider, hostContract and settings schema
HostGuard.qml, .js: host availability checks; no automatic download
Service.qml:        compatibility wrapper; native discovery uses Provider.qml directly
tests/:             generated local gate and offline Commons fixtures
assets/:            generated extension wordmark and host icon
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
