# Support and Troubleshooting

**Status: diagnostic guide for planned functionality.**

## Connection failures

Verify `kubectl config current-context` and `kubectl cluster-info`. Ensure the current kubeconfig points to the intended cluster. x9s must not bypass TLS checks or request elevated privileges.

## Missing resources

Check `kubectl api-resources` and confirm Crossplane/provider CRDs exist. Check the selected namespace and permissions (`kubectl auth can-i list <resource> --all-namespaces`). Crossplane may be installed without any managed-resource CRDs.

## Unknown or unhealthy conditions

Inspect resource `status.conditions`, Kubernetes Events, and the relevant provider controller. A missing `Ready` or `Synced` condition is not proof of success. Not all resources expose identical status fields.

## Reporting bugs

Report bugs and request features through GitHub Issues. Include x9s version, OS/terminal, Kubernetes version, Crossplane version, affected API group/kind, steps to reproduce, and sanitized logs. **Remove tokens, kubeconfigs, Secret values, and private infrastructure identifiers before sharing.**

## Current limitations

The starter contains no Kubernetes connection, discovery, or TUI yet. The first release will be read-only; log correlation and relationship navigation are later-phase features.
