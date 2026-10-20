# Usage

**Status: planned UX. Only a placeholder CLI is currently available.**

Eventually, running `x9s` will open the terminal interface using the current kubeconfig context. No in-cluster installation is required.

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
