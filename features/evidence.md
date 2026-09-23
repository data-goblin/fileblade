This file was written by an agent.

# Evidence standard

Each completed feature page pairs a screenshot with a focused MP4 of the real application. [Coverage](catalog.json) records which workflows still need capture, replacement, or visual review. A page is not release proof merely because a media file exists.

The current capture standard is:

- Native 2560×1440 output at scale 1, without stretching or synthetic desktop expansion.
- Each visible blade no wider than one-third of the full screen. The usual 650-pixel blade occupies 25.4%; the resize demonstration reaches 700 pixels, or 27.3%.
- A real Omarchy wallpaper, with idle and screensaver disabled only in the disposable capture profile.
- The complete [FileBlade SVG logo](../assets/fileblade-logo.svg), including icon and wordmark, at the bottom right. Its visible artwork has equal 32-pixel right and bottom padding.
- Continuous, muted, original-speed H.264 video, normally 4–8 seconds, showing an action and its result.

The SVG contains a mask that Qt's direct SVG rendering does not reproduce correctly. The capture overlay uses a render of the original SVG through librsvg at four times the displayed size, with its aspect ratio preserved. Cropping the SVG's transparent artboard does not change the artwork. Padding is measured from visible pixels rather than the original artboard edges.

Captures run in a separate local Hyprland compositor whose uniquely named window remains on workspace 9. Input goes to that compositor. Its home directory, sample files, session bus, and shell state are disposable. This is a local session, not an OEVM. The working desktop is not used for recording.

Sample projects, notes, images, and agent transcripts are generated fixtures. Device actions use a task-owned loop-backed image. The tailnet discovery clip omits host identities. The SFTP clip uses the explicitly chosen Omen endpoint and its real `/usr/share/doc` directory; it proves read-only browsing, with connect, refresh and disconnect checked separately.

A workflow may be exercised through its public CLI, UI, or documented backend interface. Each page identifies what the clip actually proves. Registration-only clips do not establish application handoff, and qualification-harness clips do not establish ordinary production integration.

For each accepted take, democtl validates the edit contract, renders a screenshot from the recorded frame, exports the MP4, and verifies its identity and complete decode. Visual review and behavior assertions are separate from media validity. Source commit, media hashes, dimensions, duration, and review state are recorded in coverage.

Raw takes, prior exports, capture recipes, and their contracts are retained in the task's local evidence archive under `target/feature-evidence/`. Public documentation contains the finished media and portable coverage metadata. Older icon-only or overly wide captures must be replaced before they can satisfy this standard.

Run `python3 tests/feature_evidence.py` to check complete catalogue coverage, authorship, artifact hashes, native dimensions, codec, duration and recorded visual acceptance. The command fails while any promised feature remains pending.
