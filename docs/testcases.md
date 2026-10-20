# Test Cases

**Status: acceptance plan; tests are not yet implemented.**

| ID | Scenario | Expected result | Phase |
| --- | --- | --- | --- |
| T01 | Valid kubeconfig | Connect using selected context | 1 |
| T02 | Missing/invalid kubeconfig | Actionable error, no panic | 1 |
| T03 | Cluster without Crossplane CRDs | Clear empty state | 1 |
| T04 | Dynamic provider CRD | Resource appears without hardcoded kind | 1 |
| T05 | Crossplane v1 cluster-scoped MR | Correctly listed and scoped | 1 |
| T06 | Crossplane v2 namespaced MR | Correctly listed and scoped | 1 |
| T07 | Missing `status.conditions` | Unknown state, no panic | 1 |
| T08 | Ready=True, Synced=False | Distinct condition values shown | 1 |
| T09 | RBAC forbids one API resource | Partial results and visible warning | 1 |
| T10 | Resource disappears during selection | Recover gracefully | 1 |
| T11 | Large resource list | Responsive filtering/navigation | 1 |
| T12 | Inspect YAML and Events | Correct selected resource and scope | 1 |
| T13 | XR composed resources | Tree based on actual references | 2 |
| T14 | ProviderConfig relationship | Follows explicit reference | 2 |
| T15 | Secret-bearing objects/logs | No sensitive data leaked | 1 |

Use fixtures under `tests/fixtures/crossplane-v1/` and `tests/fixtures/crossplane-v2/`. Unit tests should run without a cluster. Separate live-cluster integration tests and clearly mark their prerequisites. Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` for every change.
