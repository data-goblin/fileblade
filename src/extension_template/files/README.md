This file was written by an agent.

<p align="center"><img src="assets/fileblade-extension-logo.svg" alt="FileBlade extension" width="640"></p>

---

**FileBlade {{PLUGIN_NAME}}** adds a {{PLUGIN_NAME}} blade to [FileBlade](https://github.com/data-goblin/fileblade), which gives you IDE-like sidebars for Omarchy.

{{DESCRIPTION}}

> [!NOTE]
> Replace this note with what the extension is for and what it supports. Add a
> screenshot of the blade to `assets/` and show it above this note, the way the
> other FileBlade extensions do.

## Installation / Quick-start

Install and start the native FileBlade app first, then register this trusted extension:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/fileblade/extensions"
git clone {{REPOSITORY}} "${XDG_CONFIG_HOME:-$HOME/.config}/fileblade/extensions/{{PLUGIN_ID}}"
fileblade rescan-modules
```

Then put the blade in a slot from the blade settings, or from a terminal:

```bash
fileblade blade add right {{PLUGIN_ID}}/{{MODULE_ID}}
```

Customize this starter README to describe your extension. Full technical docs are in [docs/agent-written/README.md](docs/agent-written/README.md); design notes in [ARCHITECTURE.md](ARCHITECTURE.md).

Building your own extension is covered in FileBlade's [EXTENSIONS.md](https://github.com/data-goblin/fileblade/blob/main/EXTENSIONS.md).

## License

MIT.
