This file was written by an agent.

# Register native autostart

Opt into the native autostart role to install an owned desktop entry for login. Disable the role to reverse its owned registration.

```sh
fileblade native roles enable --role autostart
fileblade native roles status --json
```

Demonstration: The disposable profile gains an owned autostart entry pointing at the stable launcher.

The clip proves registration; it does not log out or restart the user's desktop.

Evidence reviewed.

![Register native autostart](autostart.png)

[Watch the focused demonstration](autostart.mp4) (5.97 seconds; muted, original speed).

Capture source: `c3e48bbee4f8e6c5adf0e12a3b1781ed6f6af833`. See the [evidence standard](../evidence.md) and [coverage](../catalog.json).
