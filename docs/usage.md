# Usage

**Status: planned TUI; the current executable is a placeholder and does not connect to Kubernetes.**

The executable reports its compiled package version:

```bash
x9s --version
```

The output follows `x9s MAJOR.MINOR.PATCH` (for example, `x9s 0.1.0`). The version comes from Cargo package metadata at compile time. See [development](development.md) for version bump commands.

Eventually, running `x9s` will open the terminal interface using the current kubeconfig context. No in-cluster installation is required.

The planned interface uses Ratatui for rendering and Crossterm for keyboard input and terminal events. UI rendering remains separate from Kubernetes requests; asynchronous results update the view without blocking interaction. The TUI commands below are planned and are not available in the current scaffold.

## Planned commands

| Input | Action |
| --- | --- |
| `:mr` | Managed resources |
| `:xr` | Composite resources |
| `:providers` | Providers |
| `:functions` | Functions |
| `:compositions` | Compositions |
| `:xrd` | XRDs |
| `:pc` | ProviderConfigs |
| `Enter` | Resource details |
| `Esc` | Back |
| `d` | Describe |
| `y` | YAML |
| `e` | Events |
| `/` | Filter |
| `r` | Refresh |
| `q` | Quit/back |
| `Ctrl+C` | Exit |

Phase 1 prioritizes the managed resource list, selection, details, YAML, events, and filtering. Later views may initially be unavailable. Tables should include Kind, Name, Namespace, Synced, Ready, and Age; missing conditions should display as unknown, not healthy.
