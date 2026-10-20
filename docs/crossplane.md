# Crossplane Compatibility and Resource Model

**Status: Implemented discovery and interpretation shared by the TUI and CLI.**

## Resource families

Discovery recognizes Crossplane resource families, including Managed Resources (MRs), Composite Resources (XRs), CompositeResourceDefinitions (XRDs), Compositions and CompositionRevisions, Providers and ProviderRevisions, Functions and FunctionRevisions, and ProviderConfigs. The interface in this phase is focused on confirmed MRs; family recognition does not imply a dedicated view for every family.

Provider-specific kinds and API groups are not maintained in a static list. API resources are discovered from the cluster. Scope is taken from Kubernetes discovery, supporting both cluster-scoped resources and namespaced resources, including Crossplane v2 namespaced MRs.

## Managed Resource classification

Classification returns one of three outcomes:

| Result | Meaning | Default list |
| --- | --- | --- |
| **Confirmed Managed Resource** | Available evidence supports treating the resource as an MR. | Included |
| **Possible Managed Resource** | Some evidence is consistent with an MR, but evidence is insufficient for a confident classification. | Excluded; retained internally |
| **Not a Managed Resource** | Evidence identifies another family or does not support MR classification. | Excluded |

At the CRD level, strong MR schema evidence requires both `spec.forProvider` and `status.atProvider`. That evidence confirms an MR when accompanied by any recognized Crossplane ownership or metadata convention: ownership by a `pkg.crossplane.io` ProviderRevision, ownership by an `apiextensions.crossplane.io` `ManagedResourceDefinition`, both `crossplane` and `managed` CRD categories, a `pkg.crossplane.io/` label, a `meta.crossplane.io/` annotation, or the generic Crossplane v2 managed-resource API group convention (the `.m.` group segment). These signals support custom Upjet providers whose API groups use organization-specific domains, including VCD-style groups; provider names, API groups, kinds, and CRD names are not enumerated in x9s.

Weaker schema evidence with `spec.forProvider` yields Possible. For a Possible candidate with `spec.forProvider`, object-level signals can add evidence: the `crossplane.io/external-name` annotation, `spec.providerConfigRef`, Ready/Synced conditions, and owner references. Two or more of those object-level signals promote the candidate to Confirmed. A generic `forProvider` field or an API group containing `crossplane` alone is not enough to confirm an MR. Possible candidates remain excluded from the default list, with the CRD evidence reasons available in diagnostic logs.

Only confirmed MRs appear in the default resource table. Possible MRs are retained in the internal classification/discovery results so future inspection and configuration can expose them without changing provider-specific code. This version does not provide a UI toggle for possible resources.

Classification is necessarily conservative and depends on metadata, schema, and object data exposed by the cluster. Providers differ in generated schemas and metadata, and older or customized installations may omit useful signals. A legitimate MR can therefore remain Possible and be absent from the default list. Object-level promotion does not identify arbitrary non-MR schemas from annotations or conditions alone. Classification is not proof that a provider controller is installed or healthy. A resource classified as Not may still merit investigation if its provider uses nonstandard conventions; please report a sanitized fixture when encountering one.

## Conditions and normalized resources

The shared normalized resource includes API group/version, kind, plural resource name, optional namespace, name, external name when present, and Ready and Synced conditions. Condition statuses distinguish True, False, Unknown, and Missing. Missing conditions or malformed condition entries never imply success. The CLI serializes normalized safe fields rather than the original Kubernetes object; the TUI can show available detail fields without fabricating absent values.

Discovery diagnostics are produced by the same discovery path used for listing. Independent API discovery and per-type listing failures do not discard successfully discovered resources; the CLI reports partial completion with exit code 3. A connectivity or initial CRD-list failure is fatal, not an empty result. See [usage](usage.md) for exit codes and [agent usage](agents.md) for the JSON contract.

## Compatibility limitations

- Kubernetes API discovery and CRD reads are subject to the current user's RBAC permissions. Incomplete discovery can prevent classification or listing of some APIs.
- API-level evidence can classify an installed resource family before any instances exist; object-level evidence may only be available when listing succeeds.
- Custom provider CRDs may not use a Crossplane-named API group. Classification uses schema plus ownership, metadata/category, or v2 group-convention evidence; providers that omit these signals can remain Possible.
- This release lists MRs and offers resource details. It does not yet provide XR relationship trees, YAML inspection, Events, or dedicated views for the other recognized families.
- No provider-specific kind catalog is bundled, so newly installed provider APIs do not require an x9s update.
