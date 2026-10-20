# Support and Troubleshooting

**Status: Phase 1 diagnostic guidance.**

## Connection failures

Check `kubectl config current-context` and `kubectl cluster-info` to confirm the active kubeconfig context and connectivity. x9s uses the current context unless `--context` is provided, and follows Kubernetes authentication and TLS configuration. A connectivity failure is fatal and cannot be mistaken for a successful empty resource list. Check credential plugin setup and access expiration locally; do not paste kubeconfig files or tokens into an issue.

## Missing resources

Check `kubectl api-resources` and confirm the relevant provider CRDs are installed. Verify permission to discover CRDs and list the resource (`kubectl auth can-i list <resource> --all-namespaces`). Namespace and cluster scopes come from API discovery. API discovery/listing can be incomplete when RBAC denies access.

For TUI discovery details, run x9s with a log file:

```bash
X9S_LOG_FILE=/tmp/x9s-discovery.log RUST_LOG=debug devbox run run
```

Review the trace for the active context, number of CRDs, confirmed/possible/excluded type counts, reasons a CRD was Possible, API group/version/resource matching, discovered scope, and per-type list result. A missing API group/version/plural match is logged separately from an API list error. API discovery uses a cached result per CRD group/version; a failure for one group/version is reported and does not stop discovery and listing for other groups and versions. Error records use a Kubernetes error category and avoid request bodies. If one type fails to list, x9s continues with other types. The log file is append-only; choose a path with appropriate local access controls before sharing it.

An earlier discovery hang was caused by treating an empty `metadata.continue` value as a next-page token. That caused repeated requests for the same list page. Both CRD discovery and resource listing now treat an empty continuation value as pagination completion.

Only confirmed Managed Resources appear in the default list. A resource can be a valid provider MR yet remain classified as Possible if the cluster does not expose enough classification evidence. The current interface has no control for including Possible resources. See [Crossplane classification](crossplane.md) for the evidence and limitations.

For automation, use `x9s diagnose --output json` or `x9s get managed --output json`. JSON mode keeps requested data on stdout and sends structured errors to stderr. Exit status 3 means partial discovery or listing, with resources found successfully preserved; status 1 indicates a fatal runtime error; status 2 indicates invalid arguments; status 0 indicates completion, including an empty result after completed discovery. See [agent usage](agents.md) for the output contract.

Custom Upjet providers do not need Crossplane-named API groups. A CRD with `spec.forProvider` and `status.atProvider` can be confirmed through provider ownership, Crossplane `ManagedResourceDefinition` ownership, Crossplane managed categories, package labels/metadata annotations, or the Crossplane v2 `.m.` group convention. If a CRD is Possible, its diagnostic reason list shows which evidence was observed. Do not treat a provider API group name alone as proof that a CRD is a Managed Resource.

## Unknown or unhealthy conditions

The details view shows Ready and Synced status plus available reasons and messages. A missing or malformed condition does not imply success. Not all providers expose identical status fields. Investigate provider/controller health with your established Kubernetes tools; x9s does not provide Events or provider logs in Phase 1.

## Reporting bugs

Report issues through GitHub. Include x9s version, OS/terminal, Kubernetes and Crossplane versions, affected API group/kind, sanitized steps, and relevant non-sensitive error text. Remove tokens, kubeconfigs, Secret values, and private infrastructure identifiers before sharing.

## Current limitations

x9s is read-only. The TUI lists confirmed MRs and supports details, filtering, and manual refresh; the CLI lists confirmed MRs and reports discovery diagnostics. Possible MRs are retained internally but not exposed as a list option. YAML inspection, Events, relationship navigation, provider log inspection, and dedicated views for other Crossplane families are not implemented.
