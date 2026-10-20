# Crossplane Compatibility and Resource Model

**Status: intended compatibility design.**

## Resource families

- Managed resources (provider-backed external resources)
- Composite resources (XRs), and legacy claims where installed
- CompositeResourceDefinitions (XRDs)
- Compositions and CompositionRevisions
- Providers and ProviderRevisions
- Functions and FunctionRevisions
- ProviderConfigs, whose API groups and scopes vary by provider
- Operations where available

## Crossplane v1 and v2

Crossplane installations may mix older cluster-scoped managed resources with newer namespaced resources. The UI must derive scope from Kubernetes discovery rather than guessing based on kind or API group. Composite-resource behavior and supported API versions also vary: probe what is installed rather than assuming a universal API surface.

## Classification strategy

Discover served API resources and their CRDs; identify Crossplane types using well-defined CRD/API metadata and Crossplane conventions, with test fixtures for supported variants. Treat uncertain classification as unknown. Do not label arbitrary resources as managed resources solely because they have `spec.forProvider`.

## Conditions

Parse `status.conditions` defensively. Show `Ready`, `Synced`, and other available conditions with status, reason, message, and transition time. Missing fields remain unknown. Distinguish API fetch failures from missing conditions.

## Relationships (Phase 2)

Use explicit owner references, XR `resourceRefs` or supported equivalent references, Composition references, and `providerConfigRef` fields where applicable. Avoid guessing based on names. Verify against actual fixtures before declaring v1/v2 support.
