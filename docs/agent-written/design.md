This file was written by an agent.

# FileBlade extension-system design

FileBlade is a native Omarchy application. Its built-in panes and external
extensions share navigation, focus, selection, state and bounded backend
services. Extensions add panes, file-menu actions and inventory helpers.

## Ownership

| Owner | Responsibility |
| --- | --- |
| FileBlade | Discover registered contributions, validate manifests and load compatible modules |
| Extension | Its QML, provider runtime, declared helpers and per-module behavior |
| User | Review and register trusted extensions, choose panes and enable desktop roles |
| Native installer | Verify and activate application payloads, drain updates and retain rollback state |

Register extensions under `~/.config/fileblade/extensions/<publisher.name>`.
The authority validates each manifest and records its activation. Valid new
providers are activated by default; removal and a rescan remove their
contributions. FileBlade does not download dependencies for an extension.

## Contribution contract

A manifest declares namespaced contributions under `extensions`:

- `data-goblin.fileblade/blade`: visual modules placed in a blade slot.
- `data-goblin.fileblade/action`: bounded file-menu commands.
- `data-goblin.fileblade/helper`: declared inventory reads and writes.

Module IDs include the provider identity. `hostContract` expresses the
FileBlade API version required by the module; it is separate from the
application release version. An incompatible definition may be listed but
cannot load.

One provider runtime owns shared watchers, caches and mutations. Visible
modules attach and detach, keeping only presentation and per-tab state.
A provider starts work on its first attachment and stops on its last; removal
calls its terminal shutdown hook. This prevents each view from duplicating
scans and background processes.

## Trust and execution

An accepted extension runs as the desktop user and is not sandboxed. Manifest
validation bounds input and confines entry paths; it does not establish that
QML or an executable is trustworthy.

File-menu commands run as argument vectors. The executable must belong to the
provider directory, and the backend rereads the manifest before dispatch.
Selections, output, run time and concurrency are bounded. Declared confirmation
is enforced and results enter the audit log. Inventory helpers additionally
separate permitted read and write methods and bound private standard input.

Use the host's open, edit, reveal, focus and storage services rather than
creating a parallel implementation. See [the extension guide](../../EXTENSIONS.md)
for the manifest and context APIs, and [Security](../../SECURITY.md) for the
implemented boundaries.
