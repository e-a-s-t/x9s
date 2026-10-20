# Test Cases

**Status: Phase 1 unit tests and manual acceptance scenarios.**

## Automated unit tests

Unit tests use fixtures and do not require Kubernetes:

- Upjet v1 cluster-scoped Managed Resource CRD and instance normalization.
- Custom Upjet VCD fixtures for VM, Vapp, and NATRule CRDs and instances, confirming that non-Crossplane-named API groups with Crossplane controller ownership are normalized as Managed Resources.
- VCD NATRule condition preservation (`Synced=False`, `Ready=True`) and same-kind resources remaining distinct across API groups.
- Crossplane v2 namespaced Managed Resource CRD and instance normalization.
- Non-Crossplane CRD exclusion, schema-only Possible classification, and object-level evidence promotion.
- Crossplane core family and custom Composite Resource recognition.
- Missing, malformed, True, False, and Unknown condition parsing.
- Case-insensitive filtering, deterministic sorting, and selection preservation after refresh.
- Dynamic CRD version and scope parsing.
- Dynamic API discovery matching by group, version, kind, plural, LIST capability, and API-discovered scope.
- Empty Kubernetes list continuation token ends pagination for both CRD discovery and resource listing.

Run them with `devbox run test` or as part of `devbox run ci`.

## Manual acceptance scenarios

These scenarios require a configured terminal and, where noted, a reachable cluster. They are not automated unit tests.

| ID | Scenario | Expected result | Prerequisite |
| --- | --- | --- | --- |
| M01 | Start x9s with a valid kubeconfig | Uses the active context and shows its name | Reachable cluster |
| M02 | Missing/invalid kubeconfig or auth failure | Clear error and clean TUI recovery; no panic | Terminal |
| M03 | Cluster without Crossplane MRs | Clear empty state | Reachable cluster |
| M04 | RBAC denies a discovered API | Partial results and a visible error; last successful list remains usable | Cluster with scoped RBAC |
| M05 | Refresh while selected resource remains | Selection is preserved | Reachable cluster |
| M06 | Long names, large results, and terminal resize | Table remains usable and navigation responsive | Reachable cluster |
| M07 | `--help` and `--version` | CLI exits without starting the TUI | Devbox |
| M08 | Secret-bearing objects or credential errors | Credentials and Secret values are not shown or logged | Sanitized test environment |
| M09 | Enable diagnostics with `X9S_LOG_FILE` | Discovery trace is appended to the configured file while the TUI remains readable; `RUST_LOG` controls verbosity | Writable log path and reachable cluster |
| M10 | One confirmed resource API denies or fails a list | Other confirmed resource types are still attempted; diagnostics identify the failed type and error category | Cluster with scoped RBAC or an API failure |
| M11 | Kubernetes returns an empty `metadata.continue` value | Pagination completes instead of repeatedly requesting the same page | Unit test; no cluster required |

Live-cluster behavior is verified only after x9s successfully connects to a cluster. Unit tests and local TUI startup do not establish Kubernetes connectivity.
