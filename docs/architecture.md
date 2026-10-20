# Architecture

**Status: proposed design; not yet implemented.**

## Layers

1. **Kubernetes adapter:** kube-rs client, API discovery, list/watch, error handling; uses kubeconfig and API server permissions.
2. **Crossplane interpretation:** classify discovered resources, parse conditions, extract explicit references and relationships.
3. **Domain model:** resource identity (G/V/K, scope, namespace, name, UID), conditions, metadata, and related objects.
4. **Terminal UI:** Ratatui/Crossterm navigation, filtering, tables, details, YAML and events.

Tokio tasks must keep network requests off the UI rendering path. Use bounded updates and cancellation when changing views or contexts. Never assume that every resource has `status.conditions`.

## Discovery and scope

Discover installed API resources dynamically. Classify with reliable Crossplane signals and documented API metadata; avoid a hardcoded list of provider kinds. A provider's CRDs can be installed without its controller being healthy. Respect namespace and cluster scopes and show partial results when RBAC denies some resources.

## Relationships (Phase 2)

Use explicit Crossplane owner references, composition resource references, and provider configuration references. Never infer relationships from name similarity. External cloud objects may only be represented when verifiable metadata exists.

## Security

No cluster installation. No mutation in Phase 1. Do not print Secret payloads or credentials to the screen or logs. TLS and RBAC behavior must follow Kubernetes client defaults.

## Design decision log

- **D001:** Rust + Ratatui + Crossterm for terminal UI.
- **D002:** kube-rs dynamic APIs for arbitrary provider types.
- **D003:** Read-only initial release.
- **D004:** Crossplane interpretation is separate from Kubernetes transport and UI.
