# Test Cases

**Status: Core, CLI, and TUI tests and manual acceptance scenarios.**

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
- CLI argument parsing, default TUI dispatch, output selection, namespace/context options, deterministic JSON serialization, table formatting, and documented exit-code mapping.
- CLI JSON stdout contains no terminal control sequences or logs; errors use structured stderr output; empty completed discovery is success while connectivity failure is fatal.
- Partial discovery/list failures preserve successfully discovered resources and return exit code 3.

Additional graph and TUI unit coverage:

- Stable graph identities include API version, scope, namespace, name, and UID when available.
- Typed composition, ownership, fully qualified reference, ProviderConfig, and selector edges; unresolved references and cycles remain safe.
- ProviderConfig association requires explicit identity or shared provider CRD owner UID, so same-named configs from different providers do not cross-link.
- CRD definitions, ProviderRevisions, ProviderConfigUsages, and other Crossplane control-plane objects stay out of the default infrastructure tree and remain accessible in `:system`.
- ProviderConfig schema defaults, namespaced and cluster-scoped configs, unresolved references, and v1/v2 API-model matching are covered without name-only cross-provider assignment.
- VCD's schema-declared `VM.spec.forProvider.vappName` resolves directionally to a VApp by external name and matching ProviderConfig; tree presentation displays VApp above VM.
- ProviderConfig counts deduplicate managed-resource identities; unresolved resources remain present in All Providers regardless of relationship resolution.
- Explicit tree rows retain branch depth, expand/collapse state, and selection across refresh; a known inverse `BELONGS_TO` relationship is projected as a parent-child path.
- TUI filtering, history, tree/flat selection, and refresh preserve resource identity and view state.
- YAML presentation redacts Secret payloads and credential-like values while retaining safe fields.
- ProviderConfig YAML preserves selector state, supports explicit reveal/reset, and keeps Secret references visible.

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
| M12 | Run `get managed --output json` without a TTY | JSON data only on stdout; process exits without initializing terminal UI | Devbox |
| M13 | One resource type fails discovery/listing while others succeed | Successful resources remain in JSON output, structured error is on stderr, exit code is 3 | Cluster with scoped RBAC or simulated API failure |
| M14 | Kubernetes is unreachable in JSON mode | Fatal structured error on stderr and nonzero fatal exit; never an empty success | Unreachable context |
| M15 | Completed discovery finds no instances | Empty table/JSON array and exit code 0 | Empty reachable cluster |
| M16 | Select an alternate kubeconfig context | TUI and CLI use the requested context without changing the active kubeconfig context | Two configured contexts |
| M17 | Select a ProviderConfig with the same name in another provider group | Only matching provider-owned resources are associated; unresolved resources remain accessible | Cluster with multiple providers |
| M18 | Navigate to a related resource in another ProviderConfig | The view switches to the target configuration and Esc restores the original details/list state | Cross-provider reference |
| M19 | Use `:context <name>` | Discovery reconnects to the requested kubeconfig context without exiting the TUI | Two configured contexts |
| M20 | Open YAML for Secret or credential-bearing objects | Secret payload and credential-like fields appear redacted | Fixture or test data |
| M21 | Open logs from the VCD ClusterProviderConfig | Logs show the Provider and one running controller Pod, with timestamps and live follow controls | Cluster with Provider log RBAC |
| M22 | Switch Provider Pods and return to the selector | `Tab`/`Shift+Tab` changes the displayed Pod; `Esc` cancels following and restores the selected ProviderConfig | Provider with multiple running Pods |
| M23 | Inspect ProviderConfig YAML and reveal values | Actual manifest opens redacted; `x` shows a warning and toggles values in the fetched object; reopening redacts again | Credential-bearing ProviderConfig |
| M24 | Open Provider logs from associated ProviderConfigs | Namespaced and cluster-scoped VCD configs resolve to provider-vcd; Talos resolves to provider-talos; ownership follows UID references through controller Pod | kind-local-dev with Provider log RBAC |

Live-cluster behavior is verified only after x9s successfully connects to a cluster. Unit tests and local TUI startup do not establish Kubernetes connectivity.
