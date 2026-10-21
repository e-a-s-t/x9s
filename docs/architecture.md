# Architecture

**Status: ProviderConfig navigation and relationship graph implemented; see limitations below.**

## Layers

1. **Core discovery:** kube-rs loads installed CRDs, resolves served group/version/kind/plural/scope through Kubernetes API discovery, lists CRD objects asynchronously, fetches exact authoritative referenced targets when needed, normalizes status and identity, and reports partial failures. Successful API discovery is cached on the reusable Kubernetes client between refreshes.
2. **Relationship graph:** Core builds typed edges from Crossplane composition resource references, Kubernetes owner references, fully qualified resource references, resolvable label selectors, ProviderConfig references, and schema-backed provider relationships. ProviderConfig references use explicit API identity, CRD schema kind defaults, provider ownership, and Crossplane API model context; ambiguous references stay unresolved. The graph is global and independent of TUI state.
3. **TUI:** Ratatui renders ProviderConfig selection, resource list/tree, details, YAML, Events, and describe views. Crossterm input updates navigation, history, filtering, expansion, and refresh state. Rendering does not issue Kubernetes requests; Events and refresh are asynchronous Core operations.
4. **CLI:** clap continues to expose Managed Resource listing and diagnostics using the same normalized Core report. The TUI's broader object set does not change the CLI data contract.

## Identity and discovery

Graph identity includes API version, group, kind, namespace, name, and UID when available. API discovery determines namespaced versus cluster scope; resource names and kinds are not assumed to be globally unique. Discovery lists MR, XR, ProviderConfig, and Crossplane diagnostic object instances; unrelated referenced targets are fetched by exact identity. Definitions are not treated as instances. ProviderConfigs without associated resources remain selectable. Unassigned resources have no ProviderConfig reference and unresolved references remain visible in `:all` and relationship diagnostics. Neither is a synthetic selector row.

Managed Resource classification remains conservative. Only confirmed Managed Resources are returned by the existing CLI contract. The TUI displays actual MR and XR objects plus exact referenced targets; Crossplane control-plane instances are confined to `:system`. An API or RBAC failure for one CRD is recorded as a partial discovery error while other types continue. Relationship resolution happens after listing and cannot remove an instance from `:all`.

## Relationships

The graph preserves `COMPOSES`, `MANAGED_BY`, `REFERENCES`, `OWNS`, `SELECTS`, and `BELONGS_TO` edge types. Crossplane `status.resourceRefs` and supported `spec.resourceRefs` create composition edges; owner references create ownership edges. Explicit `*Ref` and `*Refs` values resolve only when they carry API version, kind, and name. Selector edges resolve only for selectors that include API version, kind, and matchLabels. VCD's VM schema declares `forProvider.vappName` as a vApp relationship; it resolves by provider identity and external name, and the tree projects this known `BELONGS_TO` direction as VApp parent to VM child. Other scalar fields are not guessed as references.

## TUI behavior

The initial screen lists actual ProviderConfig objects only. Selector identities retain API group, kind, namespace, and name; display qualifiers disambiguate duplicate names. Counts are unique managed-resource instances, independent of edge or tree multiplicity. ProviderConfig health uses its explicit Ready condition when present and reports Unknown when health is absent. The header shows the Cargo package version and cached Kubernetes and Crossplane versions; unavailable versions display as n/a and are refreshed after context changes. Resource navigation supports tree and flat modes, filtering, details, sanitized YAML, Kubernetes Events, describe output, health filtering, command mode, and navigation history. `:system` opens the Crossplane diagnostic object inventory. YAML redacts Secret data/stringData and credential-like fields before rendering.

## Limitations

- Discovery lists installed CRDs, not every built-in Kubernetes API type. Exact built-in targets are fetched when authoritative references identify them; they can remain unresolved when target identity, namespace, discovery, or RBAC is insufficient.
- ProviderConfig reference association requires explicit reference identity or provider ownership plus compatible API model/schema defaults. If that identity is insufficient, the resource remains unresolved rather than being linked by name alone.
- Generic name-only `*Ref` fields cannot be safely mapped to target kinds without authoritative target metadata; those edges are not guessed. Generic selectors resolve only when they include target API version and kind.
- Resource tree paths use composition, ownership, and registered schema-backed directional relationships. Multi-parent objects can appear in multiple paths; recursion is guarded per path to prevent cycles.
- Endpoint values are not extracted; ProviderConfig identity is shown to avoid exposing provider-specific sensitive configuration.
- Events require RBAC to list Events in the object's namespace (or cluster-wide for cluster-scoped objects).

## Development environment

Devbox pins the Rust toolchain and development utilities. Cargo manages application dependencies and package metadata; `Cargo.toml` is the version source of truth. See [development](development.md).

Ratatui and Crossterm remain mandatory. Kubernetes transport and Crossplane interpretation stay in Core, separate from TUI and CLI presentation. The application is read-only, follows kubeconfig authentication/TLS/RBAC, and never logs credentials or Secret values.
