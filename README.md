<p align="center"><img src="assets/fileblade-logo.svg" alt="FileBlade" width="640"></p>

---

**FileBlade** is gives you IDE-like sidebars in Omarchy:
- View, search, and manage files or their properties and contents
- Helpful and malleable UI showing Git status or other information
- Drag files and hold spacebar to get an intuitive quick-action wheel
- Keyboard- or mouse-first; designed to quickly use and sheathe to get it out of your way
- Extend FileBlade to show other information or interactions in your sidebars

<p align="center"><img src="assets/fileblade-overview.gif" alt="FileBlade highlights" width="720"></p>

> [!NOTE]
> **Hello there!** I made this plugin mostly as an experiment using coding agents;
> I needed something like this to support other projects.
>
> I think it's important to be transparent that I'm from a non-technical background;
> I rely fully on coding agents for the project, but I've done my best to do my due diligence
> for planning and design, and to ensure quality and security.
> 
> I welcome any and all feedback or critique!

> [!WARNING]
> FileBlade is in 0.1.0-**beta** meaning that it's still undergoing testing and changes.
> Please only install it if you're comfortable with testing and feedback until it is out of beta.
>
> **Multiple monitors** is currently not supported in the beta (sorry!) but is planned for v0.1.0

This README is human-written. See [here](#details) for more detailed agent-written docs.

## Installation / Quick-start

FileBlade ships its bundled x86-64 Linux binary, so no Rust toolchain or
separate binary download is needed. Requires Omarchy 4.0.2 or later. Other
architectures need a compatible backend built separately.

Pick one of three routes.

**As an Omarchy plugin**, which is what most people want:

```bash
OMARCHY_SHELL_IPC_TIMEOUT=10s omarchy plugin add https://github.com/data-goblin/fileblade.git --enable
```

**As a native app, from the terminal**, the same shape as `herdr`:

```bash
curl -fsSL https://raw.githubusercontent.com/data-goblin/fileblade/main/install.sh | sh
```

That reads a signed-by-digest release manifest, checks the archive against its
SHA-256, verifies the payload inventory, and installs to `~/.local/bin`. Set
`FILEBLADE_INSTALL_MANIFEST` to install from a mirror or a local file.

**From a package**, once FileBlade is published to a repository you have
enabled:

```bash
pacman -S fileblade
```

Then, whichever route you took:

1. Restart the shell after installation finishes:

   ```bash
   omarchy restart shell
   ```

2. Add the contents of [`examples/fileblade-bindings.lua`](examples/fileblade-bindings.lua)
   to your existing `~/.config/hypr/bindings.lua`, then run `hyprctl reload`.

3. Press `Super+B` or `Super+Shift+B` to open a sidebar.

4. Hold `Super` and drag with the right mouse button over a docked blade to
   resize it, the same gesture that resizes a window. Over anything else the
   gesture still resizes the window underneath.

To remove FileBlade, remove any extensions first, then run:

```bash
omarchy plugin remove data-goblin.fileblade
omarchy restart shell
```

Your layout, settings, history and recoverable bins are retained. Remove the
FileBlade bindings you added if you no longer want them.

FileBlade has a plugin system via [extensions](EXTENSIONS.md), which are
separate Omarchy plugins. Skills, Memory, Hooks and MCP used to be example
extensions; they are built into core since 0.2.0 and need no separate install.

![Omarchy, FileBlade and FileBlade extensions](assets/docs/fileblade-extensions-simple.svg)

Click an extension to see it:

<details>
<summary><b>Agent Skills example</b></summary>
<p align="center"><img src="assets/readme/skills.png" alt="The skills blade listing plugin and marketplace skills with the agent strip" width="300"></p>
</details>

<details>
<summary><b>Agent Memory example</b></summary>
<p align="center"><img src="assets/readme/memory.png" alt="The memory blade sorted by estimated tokens" width="300"></p>
</details>

The `fileblade` CLI has a command `fileblade extension template` which you can use to quickly get a starter template btw! :)

More details below :) 

---

# FileBlade: An Omarchy Plugin

FileBlade is an IDE-style filesystem and extensible blade host for the Omarchy
Quattro shell. It provides left and right edge blades that can stay docked as
layer surfaces or become ordinary tiled Hyprland windows. Each blade contains
movable, resizable, tabbed module slots, which you can extend with a plugin system.

### FileBlade Repos

- [FileBlade core](https://github.com/data-goblin/fileblade)

The Memory, Skills, MCP and Hooks extension repositories are archived. Their
blades are built into core since 0.2.0, and they remain readable as worked
examples of a FileBlade extension.

## Features

- Left and right sidebars that open on keyboard shortcuts
- Management and navigation like a normal hyprland window
- Dock and undock sidebars
- Rust CLI `fileblade` backend which agents can also use to control and configure FileBlade
- Configurable sections of each sidebar like in an IDE
  - The default layout shows the file tree and properties
  - Bundled Notes keeps multiple named plain-text notes in any slot
  - Optional companion plugins can add Skills, Memory, MCP, Hooks, and Git modules; they are separate installs and are not installed automatically
- Selection from the filetree is shared with other modules and agents
- QoL search functionality
  - `zoxide`-like directory change
  - `fzf`-like search and deep search
  - Search syntax like -exclude "exact" and type:
  - Support for regex if you're a masochist :)
- QoL file management functionality
  - Color folders and files and adjust how colors appear
  - Undo/Redo, Cut/Copy/Paste, etc.
  - Recents, home, and trash

Example of the selection wheel:

<p align="center"><img src="assets/fileblade-drop-wheel.gif" alt="Dragging a file from FileBlade onto herdr and picking a new pane from the selection wheel" width="720"></p>

...and a lot more that I'm probably forgetting :)

Click a feature to see it:

<details>
<summary><b>FileBlade filetree</b></summary>
<p align="center"><img src="assets/readme/git-status.png" alt="The file tree with git status markers and the repository summary tooltip" width="640"></p>
</details>

<details>
<summary><b>Right-click actions and folder colours</b></summary>
<p align="center"><img src="assets/readme/folder-colors.png" alt="The row context menu with file actions and the folder color swatches" width="440"></p>
</details>

<details>
<summary><b>Searching filetree</b></summary>
<p align="center"><img src="assets/readme/deep-search.png" alt="A whole-root fzf search narrowed with type: and format: filters" width="300"></p>
</details>

<details>
<summary><b>Quick nav (like zoxide)</b></summary>
<p align="center"><img src="assets/readme/quick-nav.png" alt="The Shift+Z quick nav list of recently visited folders" width="300"></p>
</details>

## How it changes your Omarchy installation

Here's a concise list of what happens when you install FileBlade:

### Keybindings

FileBlade never writes to your Hyprland config. The binds below are the ones
you add to `~/.config/hypr/bindings.lua` (the block is in ARCHITECTURE.md under
"Focus and keybindings"); each asks FileBlade first with a short timeout and
falls back to the plain dispatcher, so nothing breaks when the shell is down.

- Super + B and Super + Shift + B open/close the left/right blades, respectively
- Super + W closes the focused sidebar, or a regular Hyprland window when no sidebar has focus
- Super + Arrows, Super + Shift + Arrows, and Super + T are also blade-aware: they focus, swap, and dock/undock a sidebar when one has focus, and otherwise behave like normal
- Super + Minus / Super + Equals resize the focused sidebar instead of the window when a sidebar has focus
- Super + Z undoes the last file operation; Shift + Z inside a file tree opens the `zoxide`-like quick nav

### Windows and focus; interaction with Hyprland

- Docked sidebars reserve their strip of the screen, so your tiled windows shift to make room
- While a sidebar has keyboard focus, the active border on your other windows is dimmed like hyprland does

### Trash and soft deletes

- Trashed files go to `~/.local/share/Trash`, so they show up in other file mgrs
- Trash is emptied automatically after **7 days by default**; change it in settings
- Things you manage through plugins (skills, memory, hooks, and so on) can be disabled instead of trashed, which moves it to `~/.local/share/fileblade/bin/` until you restore it
- Every file operation FileBlade performs is logged to `~/.local/state/omarchy/fileblade/audit.jsonl`, and undo/redo works on them

### Other

- Layout and settings live in `~/.config/omarchy/fileblade/`

## Details

This README is human-written and kept intentionally brief.
Please check the below for details which are agent-written, but I've tried to keep responsibly clear to understand for you or your agent.

- **Architecture:** See [ARCHITECTURE.md](ARCHITECTURE.md)
- **Security:** See [SECURITY.md](SECURITY.md)
- **Contributions:** See [CONTRIBUTING.md](CONTRIBUTING.md)
- **Extensions:** See [EXTENSIONS.md](EXTENSIONS.md)

## License

[MIT](LICENSE).
