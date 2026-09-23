# Configure the drop wheel

This file was written by an agent.

FileBlade reads the optional `dropWheel` member of its own `settings.json`
each time it opens the wheel and each time it dispatches a custom command.
The containing settings document keeps its existing version. The wheel member
has its own `version: 1`. An absent member uses the standard wheel.

The current plugin profile stores this document at
`$XDG_CONFIG_HOME/omarchy/fileblade/settings.json` (normally
`~/.config/omarchy/fileblade/settings.json`). The native profile uses the
preferences path selected by the native runtime. Edit the existing document,
preserving its other members. Ordinary preference saves preserve unknown
members, including unknown fields inside `dropWheel`.

The core settings form entry is an integration dependency. This schema is
usable by editing the settings document now; it does not claim a completed
visual wheel editor.

## Order, visibility and appearance

`actions` is an ordered array of overrides. Listed available actions appear
first, followed by unlisted available actions in their original order.
Custom actions can be included in this order by their `custom:` ids. Their
definitions belong in `customActions`. Unlisted custom actions follow the
standard actions. `hidden: true` removes an action. To restore it, remove
that override or set `hidden: false`.

Built-in ids are `open`, `open-with`, `terminal`, `review`, `mux-open`,
`nvim-open`, and `app-open`. Availability still depends on the target:
configuration cannot make an unavailable multiplexer or editor work.
`application` and `copy-paths` are also reserved dispatch ids.

`terminal` opens files in nvim in a new terminal, or a shell for a folder-only
selection. `review` stays visible but disabled unless the selected Git paths
have staged, unstaged or untracked changes. Dispatch checks status again.
Herdr target discovery queries the live focused pane instead of inherited
pane identifiers from processes in inactive tabs.

An override can set `label`, `key`, `glyph`, `icon`, `description`, `group`,
and `placements`. `key` is one ASCII letter or digit, or empty for automatic
assignment. Keys are unique within each ring; a duplicate receives an
available key. Groups are descriptive metadata; array order determines the
wheel order.

`icon` is a theme icon name such as `utilities-terminal`, or a bundled mark
(`herdr`, `tmux`, `pane-horizontal`, `pane-vertical`). The name may contain
ASCII letters, digits, hyphens, underscores and dots. Paths and URLs are
not accepted. An explicit icon clears the inherited icon source and mask,
so it wins over an application-resolved icon. An explicit `glyph` without
`icon` clears the inherited icon and uses the glyph instead. The renderer
bounds the icon size. Existing application icon fields and bundled marks
remain usable without configuration.

`placements` works the same way for each action's children: listed children
come first, unlisted children remain, and `hidden` removes a child. Hiding
all children removes the containing action. Multiplexer placement ids are:

| Target | Placement ids |
| --- | --- |
| herdr | `right`, `down`, `tab`, `workspace` |
| tmux | `right`, `down`, `window`, `session` |
| nvim | `tab`, `buffer`, `split` |
| hunk review over a multiplexer | `right`, `down`, `tab`, `workspace`, `window` |

Open-with application placements share the internal id `application`.
Identify an override for one application by `desktop_id`, for example
`{"desktop_id":"org.gnome.Loupe.desktop","hidden":true}`.

## Custom actions and two placement layers

A custom action has a unique `id` starting with `custom:`, a nonempty
`label`, optional display fields listed above, and either a `command`,
a `builtin` reference, or executable descendants in `placements`.
Child ids are unique within their parent. A child added to a built-in's
placements must also start with `custom:`. Descendants defined inside a
custom action may use short ids such as `numbered`.

There are at most three rings: action, placement, sub-placement. Each level
can have its own icon, key, label and command. An entry with children opens
its next ring; its own command is not executed until it is a leaf. There is
no implicit command inheritance: each executable child declares its own
command or built-in reference.

To reuse an available built-in as a custom leaf, use:

```json
{
  "id": "custom:split",
  "label": "Split here",
  "builtin": {"action": "mux-open", "placement": "right"}
}
```

A reference inherits the built-in glyph and icon unless explicitly overridden.
It must identify an executable built-in leaf. Omit `placement` for
an action without children, such as `terminal`. For an Open-with reference,
use `action: "open-with"` and the desktop id as `placement`.
Built-in implementations cannot be replaced by adding `command` to an
override. Add a separate custom action instead.

`targetKinds` optionally restricts a custom action to a nonempty array drawn
from `desktop`, `terminal`, `editor`, `window`, `blade`. Applications and
browsers are normalized to `window`. The `blade` value applies when the
runtime supplies a blade target; the legacy desktop drop resolver currently
excludes its own blade windows. Omission allows every target, subject to
run-mode capability checks.

`conditions` optionally contains `mime` and `path` arrays of patterns. Within
an array, any pattern may match; across both arrays, every condition must
match every selected path. `*` matches any sequence except a newline; other characters are
literal. Examples: `"mime":["image/*"]`, `"path":["/home/me/project/*.rs"]`.
Path matching is case-sensitive against the full path representation. The
existing resolver probes at most 12 MIME values. An unprobed or unknown MIME
never satisfies a MIME condition.

## Commands

`command` is an argument array with a literal executable first. FileBlade
does not interpret it as a shell command. For example:

```json
["my-inspector", "--", "{paths}"]
```

Substitutions occupy whole arguments:

| Argument | Expansion |
| --- | --- |
| `{paths}` | Every selected path as a separate argument |
| `{path}` | The sole selected path; multiple selections are refused |
| `{cwd}` | The selected folder, or the first selected file's parent |
| `{git_root}` | The common detected repository root; absence is refused |

Path bytes are retained through native argument construction, including
FileBlade's encoded representation for non-UTF-8 paths. Quotes, spaces,
`$()` and shell operators in substituted paths stay data. An embedded
substitution such as `--file={path}` is rejected; use two arguments.
Unrecognized brace expressions are rejected. Include `--` where the chosen
program needs an end-of-options delimiter; FileBlade does not infer the
option syntax of arbitrary programs.

`runMode` defaults to `detached`:

| Mode | Behavior |
| --- | --- |
| `detached` | Launch the argv with `{cwd}` as working directory |
| `terminal` | Use the desktop's terminal launcher with that directory and argv |
| `multiplexer` | Use the resolved herdr/tmux target, with required `placement` from its table above |

Terminal and multiplexer transport pass a fixed launcher with byte-encoded arguments
and working directory. That launcher decodes the payload and executes the
argument array; the configured command never becomes shell program text.
The fixed decoder is the FileBlade binary itself, invoked by absolute path as
`fileblade exec-hex`, so no interpreter and no PATH entry is required.
Literal file URIs, empty arguments and non-UTF-8 path bytes retain their
meaning in every run mode. Ambiguous or stale targets are refused. Custom
commands and custom built-in references are resolved from current settings
and current file facts again on dispatch. Removing, hiding or making an entry
inapplicable invalidates an old wheel's route.

## Worked example

Merge this member into the existing settings document. It puts Inspect first,
renames the terminal action, removes Open with, and supplies three executable
leaves across two placement layers:

```json
{
  "dropWheel": {
    "version": 1,
    "actions": [
      {"id": "custom:inspect"},
      {"id": "terminal", "label": "Shell", "key": "s", "icon": "utilities-terminal"},
      {"id": "open-with", "hidden": true}
    ],
    "customActions": [
      {
        "id": "custom:inspect",
        "label": "Inspect",
        "key": "i",
        "icon": "document-properties",
        "targetKinds": ["desktop", "terminal", "editor", "window"],
        "conditions": {"mime": ["text/*"]},
        "placements": [
          {
            "id": "text",
            "label": "Text",
            "key": "t",
            "placements": [
              {"id": "plain", "label": "Read", "key": "r", "command": ["less", "--", "{paths}"], "runMode": "terminal"},
              {"id": "numbered", "label": "Line numbers", "key": "n", "command": ["less", "-N", "--", "{paths}"], "runMode": "terminal"}
            ]
          },
          {"id": "custom:shell", "label": "Shell here", "key": "s", "builtin": {"action": "terminal"}}
        ]
      }
    ]
  }
}
```

Letters choose the displayed action, then placement, then sub-placement.
Arrows and Tab move within the active ring; Enter accepts. Escape or Backspace
returns one layer, then closes the wheel. Pointer hover exposes children;
clicking an executable wedge dispatches it.

## Limits and diagnostics

The existing preferences document limit is 64 KiB. Each ring displays at most
12 entries, custom definitions consume a budget of 96 nodes, and a fourth
ring is rejected. Commands have at most 128 arguments, each at most 4096
bytes. Display text is bounded to 512 bytes and cannot contain control
characters. Identifiers are at most 96 bytes.

A malformed custom entry is skipped; a malformed override is ignored and its
built-in remains available. The wheel footer shows the diagnosis. Full
messages are returned in the drop-context response's `diagnostics` array.
An unsupported wheel schema version uses defaults and reports the problem.
No wheel read rewrites the settings document or drops unknown fields.

A drag released while the wheel is loading is remembered for up to 800 ms.
Rows arriving within that interval activate at the remembered point once.
After the interval, the wheel stays open for an explicit choice. Closing the
wheel discards the pending activation.

## Settings form integration reference

The schema is implemented in commit `c7f3199`. The settings owner adds the
form entry under the existing preferences path. Suggested label: **Drop
wheel**. Suggested description: **Choose actions, their order and icons,
and commands with up to two layers of sub-actions.** Store an object under
`dropWheel`, not a JSON string. An absent member restores the standard wheel;
an explicit empty configuration is `{"version":1,"actions":[],"customActions":[]}`.

| Field | Type and default | Editing rule |
| --- | --- | --- |
| `version` | Integer, `1` | Wheel schema version; leave the containing settings version unchanged |
| `actions` | Array, omitted means no overrides | Ordered references to available built-ins or defined custom actions |
| `customActions` | Array, omitted means none | Custom root definitions; at most 12 definitions are considered |
| `id` | String | Stable action/child identity; custom ids use ASCII letters, digits, `-`, `_`, `:`, at most 96 bytes; root and added built-in child ids start with `custom:` |
| `desktop_id` | String | Select an existing Open-with application override; takes precedence over `id` for matching |
| `hidden` | Boolean, `false` | Hide the entry without deleting its definition |
| `label` | String | Required and nonblank for custom definitions; overrides inherit when omitted |
| `key` | String, automatic when empty or omitted on custom definitions | One ASCII letter or digit; collisions receive an available key |
| `glyph` | String | Text fallback; an explicit glyph without an icon replaces inherited imagery |
| `icon` | String | Theme name or bundled mark name; explicit value replaces inherited imagery |
| `description` | String, empty for custom definitions | Description displayed by the wheel |
| `group` | String, `custom` for custom definitions | Descriptive metadata; does not control ordering |
| `placements` | Array | Ordered child overrides or custom definitions; allow action → placement → sub-placement only |
| `targetKinds` | Nonempty string array, omitted means all eligible kinds | Custom definitions only: `desktop`, `terminal`, `editor`, `window`, `blade` |
| `conditions` | Object, omitted means unrestricted | Custom definitions only; optional `mime` and `path` arrays |
| `conditions.mime`, `conditions.path` | Nonempty string arrays | Each has at most 12 patterns, each nonempty and at most 256 bytes |
| `command` | String array | Custom executable leaf or container fallback; 1–128 arguments, each at most 4096 bytes, no NUL; first argument is a literal executable |
| `runMode` | String, `detached` | Applies to `command`: `detached`, `terminal`, or `multiplexer` |
| `placement` | String | Required for a multiplexer command; use the resolved target's placement ids above |
| `builtin` | Object | Custom reference to an available executable built-in; mutually exclusive with `command` |
| `builtin.action` | String | Required built-in action id |
| `builtin.placement` | String, omitted for a leaf action | Placement id, or desktop id for an Open-with application |

The six display strings (`label`, `key`, `glyph`, `icon`, `description`,
`group`) are bounded to 512 bytes and reject control characters; the stricter
key/icon rules still apply. Omitted override fields inherit existing values.
Custom descendants declare their own conditions and commands; ancestor
conditions also restrict whether their branch exists. If a command-bearing
container has no remaining visible children, its own command becomes the
executable leaf. Use command-free containers when that fallback is unwanted.
Built-in overrides accept display, visibility and child edits; put execution
and applicability fields in custom definitions.

Preserve unknown members in the original settings object, including inside
individual definitions, when saving an edit. Do not serialize the rendered
wheel rows back into settings: they contain generated keys and dispatch
metadata rather than the user's definitions. Reset only the `dropWheel`
member. Keep the existing whole-document 64 KiB limit.

The existing pure Rust entry point is
`fileblade::drop_target::config::merge(defaults, document, target, facts)`.
It accepts `&[serde_json::Value]` followed by three `&serde_json::Value`
arguments and returns `(Vec<serde_json::Value>, Vec<String>)`: visible rows
and diagnostics. `document` is the wheel member itself, not all settings.
Supply the current built-in rows and target/file facts for a contextual
preview. This is not a standalone save validator: unavailable actions and
conditions depend on that context. The lane adds no preference-write API or
settings-form implementation; those remain with the settings owner.

All three wheel rings consume
`FileIcons.resolveApplication(entry, override, desktopEntry, iconPath)` when
available. Application rows carry their desktop identity, which the wheel
looks up through Quickshell DesktopEntries; explicit icon or glyph overrides
are passed separately so they take precedence. The returned `icon`,
`icon_source` and `glyph` feed the existing bounded icon renderer. Built-in
action ids remain stable when desktop identity is present.

Application icon resolution is present in the 0.2.0 shared resolver. The
wheel uses it for application rows when the guest provides the interface and
retains its existing icon/source behavior for legacy rows; an older guest
without resolver support leaves the probe cases pending. Bundled herdr/tmux
and pane marks remain available with either path; this baseline has no bundled
hunk mark, so hunk retains its glyph fallback.

Guest acceptance runs through the plain `OVM` interface. Section 20 exercises
section and tab arrangement, section 26 exercises drag selection, and
section 38 exercises configured commands and stale-route rejection. Their
screenshots carry expectation numbers; a screenshot does not turn a pending
assertion into a pass.

For installed-native qualification, delivery owns `tests/vm/native-ovm`
(R64). Set `OVM` to that executable, `FILEBLADE_SHAPE=native`, and `SKIP_PUSH=1`
against the prepared installation. Shared `lib.sh` helpers select the
control/backend route (R65); the adapter forwards the published commands.
The individual scenarios contain no native routing branch. Native
results must bind the active payload and activation receipt, and must be
rerun even when the same scenarios passed against the plugin.

`tests/vm/expectations/26-wheel-icons.sh` loads the guest's production
DropWheel and icon renderer in a disposable Quickshell probe. It uses real
DesktopEntries/theme lookup and checks the renderer portion of E-39-01 on a
Neovim row against its launcher icon, without invoking the application; the
explicit herdr override remains E-38-03, the missing desktop entry checks the
bundled-mark renderer portion of E-39-02, and the missing image checks the
legacy fallback-glyph portion related to E-39-06. That fixture still supplies
a fake desktop id, so it does not qualify E-39-06's unnamed-icon and no-catalogue
behaviour. E-39-03, E-39-04 and E-39-05 remain unqualified by this probe. Each
case has a labelled screenshot. Missing resolver support reports pending cases.

The probe supplies row descriptors and a static controller fixture; it does
not qualify drag dispatch or every wheel ring. Failed-image retry through
the renderer's application descriptor API remains separate from the
wheel's current icon/source forwarding. A source overlay used to prepare
these checks is not evidence for a merged or installed-native candidate.
