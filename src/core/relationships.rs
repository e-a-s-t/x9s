use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::models::{Resource, ResourceId};

#[path = "relationships/vcd.rs"]
mod vcd;

pub fn vcd_vapp_name_reference(group: &str, kind: &str, schema: &Value) -> bool {
    vcd::schema_declares_vapp_name_relationship(group, kind, schema)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RelationshipType {
    Composes,
    ManagedBy,
    References,
    Owns,
    Selects,
    BelongsTo,
}

impl RelationshipType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Composes => "COMPOSES",
            Self::ManagedBy => "MANAGED_BY",
            Self::References => "REFERENCES",
            Self::Owns => "OWNS",
            Self::Selects => "SELECTS",
            Self::BelongsTo => "BELONGS_TO",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relationship {
    pub from: ResourceId,
    pub to: Option<ResourceId>,
    pub relation: RelationshipType,
    pub unresolved: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RelationshipGraph {
    pub nodes: HashSet<ResourceId>,
    pub relationships: Vec<Relationship>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetReference {
    pub api_version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

/// Exact identities carried by Crossplane composition and fully-qualified
/// resource-reference metadata. Name-only references are intentionally omitted.
pub fn authoritative_targets(resources: &[Resource]) -> Vec<TargetReference> {
    fn collect(value: &Value, namespace: Option<&str>, output: &mut Vec<TargetReference>) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if key.ends_with("Ref") || key.ends_with("Refs") || key == "resourceRefs" {
                        let references: Vec<_> = child
                            .as_array()
                            .map(|items| items.iter().collect())
                            .unwrap_or_else(|| vec![child]);
                        for reference in references {
                            if let (Some(api_version), Some(kind), Some(name)) = (
                                reference.get("apiVersion").and_then(Value::as_str),
                                reference.get("kind").and_then(Value::as_str),
                                reference.get("name").and_then(Value::as_str),
                            ) {
                                output.push(TargetReference {
                                    api_version: api_version.to_owned(),
                                    kind: kind.to_owned(),
                                    namespace: reference
                                        .get("namespace")
                                        .and_then(Value::as_str)
                                        .map(str::to_owned)
                                        .or_else(|| namespace.map(str::to_owned)),
                                    name: name.to_owned(),
                                });
                            }
                        }
                    }
                    if key != "ownerReferences" && key != "providerConfigRef" {
                        collect(child, namespace, output);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect(item, namespace, output);
                }
            }
            _ => {}
        }
    }
    let mut targets = HashSet::new();
    for resource in resources {
        for path in ["/status/resourceRefs", "/spec/resourceRefs"] {
            if let Some(references) = resource.original.pointer(path).and_then(Value::as_array) {
                for reference in references {
                    if let (Some(api_version), Some(kind), Some(name)) = (
                        reference.get("apiVersion").and_then(Value::as_str),
                        reference.get("kind").and_then(Value::as_str),
                        reference.get("name").and_then(Value::as_str),
                    ) {
                        targets.insert(TargetReference {
                            api_version: api_version.to_owned(),
                            kind: kind.to_owned(),
                            namespace: reference
                                .get("namespace")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                                .or_else(|| resource.namespace.clone()),
                            name: name.to_owned(),
                        });
                    }
                }
            }
        }
        let mut collected = Vec::new();
        collect(
            &resource.original,
            resource.namespace.as_deref(),
            &mut collected,
        );
        targets.extend(collected);
    }
    let mut targets: Vec<_> = targets.into_iter().collect();
    targets.sort_by(|left, right| {
        (&left.api_version, &left.kind, &left.namespace, &left.name).cmp(&(
            &right.api_version,
            &right.kind,
            &right.namespace,
            &right.name,
        ))
    });
    targets
}

impl RelationshipGraph {
    pub fn build(resources: &[Resource]) -> Self {
        let mut graph = Self::default();
        let mut resource_map: HashMap<ResourceId, &Resource> = HashMap::new();
        let mut by_address: HashMap<(String, String, Option<String>, String), ResourceId> =
            HashMap::new();
        let mut by_name: HashMap<String, Vec<ResourceId>> = HashMap::new();
        for resource in resources {
            let id = resource.id();
            by_address.insert(
                (
                    id.api_version.clone(),
                    id.kind.clone(),
                    id.namespace.clone(),
                    id.name.clone(),
                ),
                id.clone(),
            );
            by_name.entry(id.name.clone()).or_default().push(id.clone());
            graph.nodes.insert(id.clone());
            resource_map.insert(id, resource);
        }

        for (id, resource) in &resource_map {
            let metadata = resource.original.get("metadata").unwrap_or(&Value::Null);
            if let Some(owners) = metadata.get("ownerReferences").and_then(Value::as_array) {
                for owner in owners {
                    let (Some(api), Some(kind), Some(name)) = (
                        owner.get("apiVersion").and_then(Value::as_str),
                        owner.get("kind").and_then(Value::as_str),
                        owner.get("name").and_then(Value::as_str),
                    ) else {
                        continue;
                    };
                    let owner_uid = owner.get("uid").and_then(Value::as_str);
                    let target = resource_map
                        .keys()
                        .find(|candidate| {
                            candidate.api_version == api
                                && candidate.kind == kind
                                && candidate.name == name
                                && (candidate.namespace == id.namespace
                                    || candidate.namespace.is_none())
                                && owner_uid.is_none_or(|uid| candidate.uid.as_deref() == Some(uid))
                        })
                        .cloned();
                    let unresolved = target.is_none().then(|| format!("{kind}/{name}"));
                    graph.relationships.push(Relationship {
                        from: target.clone().unwrap_or_else(|| id.clone()),
                        to: target.map(|_| id.clone()),
                        relation: RelationshipType::Owns,
                        unresolved,
                    });
                }
            }

            // Crossplane records the authoritative composed object identities on XRs.
            for path in ["/status/resourceRefs", "/spec/resourceRefs"] {
                if let Some(refs) = resource.original.pointer(path).and_then(Value::as_array) {
                    for reference in refs {
                        if let Some(edge) =
                            explicit_edge(id, reference, &by_address, RelationshipType::Composes)
                        {
                            graph.relationships.push(edge);
                        }
                    }
                }
            }

            let mut references = Vec::new();
            collect_reference_edges(
                &resource.original,
                id,
                &by_address,
                &resource_map,
                &mut references,
            );
            graph.relationships.extend(references);

            // Resolve only fully qualified ProviderConfig references. Common name-only
            // references stay visible as unresolved rather than crossing provider groups.
            if let Some(reference) = resource.original.pointer("/spec/providerConfigRef") {
                if let Some(name) = reference.get("name").and_then(Value::as_str) {
                    let api = reference.get("apiVersion").and_then(Value::as_str);
                    let kind = reference
                        .get("kind")
                        .and_then(Value::as_str)
                        .or(resource.provider_config_default_kind.as_deref());
                    let reference_namespace = reference.get("namespace").and_then(Value::as_str);
                    let provider_owner_uid = resource.provider_owner_uid.as_deref();
                    let candidates: Vec<_> = by_name
                        .get(name)
                        .into_iter()
                        .flatten()
                        .filter(|candidate| {
                            resource_map.get(*candidate).is_some_and(|target| {
                                target.family.label() == "ProviderConfig"
                                    && if let Some(namespace) = reference_namespace {
                                        target.namespace.as_deref() == Some(namespace)
                                    } else if let Some(namespace) = id.namespace.as_deref() {
                                        target.namespace.as_deref().is_none_or(|target_namespace| {
                                            target_namespace == namespace
                                        })
                                    } else {
                                        target.namespace.is_none()
                                    }
                                    && api.is_none_or(|api| target.api_version == api)
                                    && kind.is_none_or(|kind| target.kind == kind)
                                    && (api.is_some()
                                        || is_crossplane_v2_group(&id.group)
                                            == is_crossplane_v2_group(&target.group))
                                    && (api.is_some() && kind.is_some()
                                        || provider_owner_uid.is_some()
                                            && target.provider_owner_uid.as_deref()
                                                == provider_owner_uid)
                            })
                        })
                        .cloned()
                        .collect();
                    let unresolved = match candidates.len() {
                        1 => None,
                        0 if api.is_some() && kind.is_some() => Some(format!(
                            "{} {}/{} was not found in the referenced namespace/scope",
                            kind.unwrap_or("ProviderConfig"), api.unwrap_or_default(), name
                        )),
                        0 if provider_owner_uid.is_none() => Some(format!(
                            "ProviderConfig {name} could not be safely resolved: provider ownership identity and API version are unavailable"
                        )),
                        0 => Some(format!(
                            "No ProviderConfig kind/name {}/{} exists for this provider installation",
                            kind.unwrap_or("(schema default missing)"), name
                        )),
                        _ => Some(format!(
                            "ProviderConfig reference {}/{} is ambiguous across API groups or scopes",
                            kind.unwrap_or("(kind omitted)"), name
                        )),
                    };
                    graph.relationships.push(Relationship {
                        from: id.clone(),
                        to: (candidates.len() == 1).then(|| candidates[0].clone()),
                        relation: RelationshipType::ManagedBy,
                        unresolved,
                    });
                } else {
                    graph.relationships.push(Relationship {
                        from: id.clone(),
                        to: None,
                        relation: RelationshipType::ManagedBy,
                        unresolved: Some("providerConfigRef is missing name".into()),
                    });
                }
            }
        }

        // Provider-specific relationship rules are isolated from generic graph
        // construction and run only after all ordinary identity edges exist.
        let provider_relationships = vcd::resolve(resources, &graph.relationships);
        graph.relationships.extend(provider_relationships);

        let mut seen = HashSet::new();
        graph.relationships.retain(|edge| {
            seen.insert((
                edge.from.clone(),
                edge.to.clone(),
                edge.relation,
                edge.unresolved.clone(),
            ))
        });
        graph
    }
}

fn is_crossplane_v2_group(group: &str) -> bool {
    group.split('.').any(|part| part == "m")
}

fn collect_reference_edges(
    value: &Value,
    from: &ResourceId,
    by_address: &HashMap<(String, String, Option<String>, String), ResourceId>,
    resources: &HashMap<ResourceId, &Resource>,
    edges: &mut Vec<Relationship>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "ownerReferences" || key == "resourceRefs" || key == "providerConfigRef" {
                    // These fields have dedicated handling or incomplete identity.
                } else if key.ends_with("Ref") || key.ends_with("Refs") {
                    let values: Vec<&Value> = child
                        .as_array()
                        .map(|items| items.iter().collect())
                        .unwrap_or_else(|| vec![child]);
                    for reference in values {
                        if let Some(edge) =
                            explicit_edge(from, reference, by_address, RelationshipType::References)
                        {
                            edges.push(edge);
                        }
                    }
                } else if key.ends_with("Selector") {
                    let api = child.get("apiVersion").and_then(Value::as_str);
                    let kind = child.get("kind").and_then(Value::as_str);
                    let labels = child.pointer("/matchLabels").and_then(Value::as_object);
                    if let (Some(api), Some(kind), Some(labels)) = (api, kind, labels) {
                        let mut matches_found = false;
                        for candidate in by_address.values().filter(|candidate| {
                            candidate.api_version == api && candidate.kind == kind
                        }) {
                            let matches = candidate_matches_labels(candidate, labels, resources);
                            if matches {
                                matches_found = true;
                                edges.push(Relationship {
                                    from: from.clone(),
                                    to: Some(candidate.clone()),
                                    relation: RelationshipType::Selects,
                                    unresolved: None,
                                });
                            }
                        }
                        if !matches_found {
                            edges.push(Relationship {
                                from: from.clone(),
                                to: None,
                                relation: RelationshipType::Selects,
                                unresolved: Some(key.clone()),
                            });
                        }
                    } else {
                        edges.push(Relationship {
                            from: from.clone(),
                            to: None,
                            relation: RelationshipType::Selects,
                            unresolved: Some(key.clone()),
                        });
                    }
                }
                collect_reference_edges(child, from, by_address, resources, edges);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_reference_edges(item, from, by_address, resources, edges);
            }
        }
        _ => {}
    }
}

fn candidate_matches_labels(
    candidate: &ResourceId,
    selector: &serde_json::Map<String, Value>,
    resources: &HashMap<ResourceId, &Resource>,
) -> bool {
    let labels = resources
        .get(candidate)
        .and_then(|resource| resource.original.pointer("/metadata/labels"))
        .and_then(Value::as_object);
    selector
        .iter()
        .all(|(key, value)| labels.and_then(|labels| labels.get(key)) == Some(value))
}

fn explicit_edge(
    from: &ResourceId,
    reference: &Value,
    by_address: &HashMap<(String, String, Option<String>, String), ResourceId>,
    relation: RelationshipType,
) -> Option<Relationship> {
    let api = reference.get("apiVersion").and_then(Value::as_str)?;
    let kind = reference.get("kind").and_then(Value::as_str)?;
    let name = reference.get("name").and_then(Value::as_str)?;
    let explicit_namespace = reference
        .get("namespace")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let namespace = explicit_namespace
        .clone()
        .or_else(|| from.namespace.clone());
    let address = (api.to_owned(), kind.to_owned(), namespace, name.to_owned());
    let to = by_address.get(&address).cloned().or_else(|| {
        if explicit_namespace.is_some() {
            return None;
        }
        by_address
            .get(&(api.to_owned(), kind.to_owned(), None, name.to_owned()))
            .cloned()
    });
    Some(Relationship {
        from: from.clone(),
        to: to.clone(),
        relation,
        unresolved: to.is_none().then(|| format!("{api} {kind}/{name}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        crossplane::{normalize, CrdInfo},
        models::ResourceFamily,
    };
    use serde_json::json;

    fn resource(
        group: &str,
        kind: &str,
        name: &str,
        family: ResourceFamily,
        original: Value,
        owner_uid: Option<&str>,
    ) -> Resource {
        let owners = owner_uid.map(|uid| json!([{"apiVersion":"pkg.crossplane.io/v1","kind":"ProviderRevision","name":"provider-r1","uid":uid}])).unwrap_or_else(|| json!([]));
        let crd = CrdInfo {
            group: group.into(),
            kind: kind.into(),
            namespaced: false,
            schema: json!({}),
            labels: json!({}),
            annotations: json!({}),
            owner_references: owners,
            categories: vec![],
            family,
            api_version: format!("{group}/v1"),
            plural: format!("{}s", kind.to_lowercase()),
            provider_config_default_kind: None,
            vcd_vapp_name_reference: false,
        };
        let mut value = original;
        value["apiVersion"] = json!(format!("{group}/v1"));
        value["kind"] = json!(kind);
        value["metadata"]["name"] = json!(name);
        normalize(value, &crd)
    }

    #[test]
    fn resolves_composition_and_typed_resource_references() {
        let parent = resource(
            "xr.example.org",
            "Cluster",
            "root",
            ResourceFamily::CompositeResource,
            json!({"status":{"resourceRefs":[{"apiVersion":"infra.example.org/v1","kind":"Machine","name":"node"}]},"spec":{"networkRef":{"apiVersion":"net.example.org/v1","kind":"Network","name":"lan"}}}),
            None,
        );
        let node = resource(
            "infra.example.org",
            "Machine",
            "node",
            ResourceFamily::ManagedResource,
            json!({}),
            None,
        );
        let network = resource(
            "net.example.org",
            "Network",
            "lan",
            ResourceFamily::Other,
            json!({}),
            None,
        );
        let targets = authoritative_targets(&[parent.clone()]);
        assert!(targets
            .iter()
            .any(|target| target.kind == "Machine" && target.name == "node"));
        let graph = RelationshipGraph::build(&[parent, node, network]);
        assert!(graph
            .relationships
            .iter()
            .any(|edge| edge.relation == RelationshipType::Composes
                && edge.to.as_ref().is_some_and(|id| id.name == "node")));
        assert!(graph
            .relationships
            .iter()
            .any(|edge| edge.relation == RelationshipType::References
                && edge.to.as_ref().is_some_and(|id| id.name == "lan")));
    }

    #[test]
    fn resolves_provider_config_only_with_shared_provider_identity() {
        let mr = resource(
            "s3.aws.example.org",
            "Bucket",
            "bucket",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"home"}}}),
            Some("revision-uid"),
        );
        let pc = resource(
            "aws.example.org",
            "ProviderConfig",
            "home",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-uid"),
        );
        let graph = RelationshipGraph::build(&[mr, pc]);
        assert!(graph
            .relationships
            .iter()
            .any(|edge| edge.relation == RelationshipType::ManagedBy
                && edge.to.as_ref().is_some_and(|id| id.name == "home")));

        let mr = resource(
            "s3.aws.example.org",
            "Bucket",
            "bucket",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"home"}}}),
            None,
        );
        let pc = resource(
            "aws.example.org",
            "ProviderConfig",
            "home",
            ResourceFamily::ProviderConfig,
            json!({}),
            None,
        );
        let graph = RelationshipGraph::build(&[mr, pc]);
        assert!(graph
            .relationships
            .iter()
            .any(|edge| edge.relation == RelationshipType::ManagedBy && edge.to.is_none()));
        assert!(graph
            .relationships
            .iter()
            .find(|edge| edge.relation == RelationshipType::ManagedBy)
            .and_then(|edge| edge.unresolved.as_deref())
            .is_some_and(|reason| reason.contains("could not be safely resolved")));
    }

    #[test]
    fn same_named_provider_configs_stay_with_their_provider_revision() {
        let mr = resource(
            "compute.alpha.example.org",
            "Machine",
            "node",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"home"}}}),
            Some("revision-a"),
        );
        let alpha = resource(
            "alpha.example.org",
            "ProviderConfig",
            "home",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-a"),
        );
        let beta = resource(
            "beta.example.org",
            "ProviderConfig",
            "home",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-b"),
        );
        let alpha_id = alpha.id();
        let beta_id = beta.id();
        let graph = RelationshipGraph::build(&[mr, alpha, beta]);
        let edge = graph
            .relationships
            .iter()
            .find(|edge| edge.relation == RelationshipType::ManagedBy)
            .unwrap();
        assert_eq!(edge.to.as_ref(), Some(&alpha_id));
        assert_ne!(edge.to.as_ref(), Some(&beta_id));
    }

    #[test]
    fn resolves_vcd_vapp_to_vm_from_schema_hint_and_provider_identity() {
        let vapp = resource(
            "vcd.vcd.m.carekube.com",
            "Vapp",
            "seed",
            ResourceFamily::ManagedResource,
            json!({"metadata":{"annotations":{"crossplane.io/external-name":"seed"}},"spec":{"providerConfigRef":{"name":"default","kind":"ClusterProviderConfig"}}}),
            Some("revision-vcd"),
        );
        let mut vm = resource(
            "vapp.vcd.m.carekube.com",
            "VM",
            "seed-cp01",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"default","kind":"ClusterProviderConfig"},"forProvider":{"vappName":"seed"}}}),
            Some("revision-vcd"),
        );
        vm.provider_config_default_kind = Some("ClusterProviderConfig".into());
        vm.vcd_vapp_name_reference = true;
        let config = resource(
            "vcd.m.carekube.com",
            "ClusterProviderConfig",
            "default",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-vcd"),
        );
        let vapp_id = vapp.id();
        let vm_id = vm.id();
        let graph = RelationshipGraph::build(&[vm, config, vapp]);
        assert!(graph.relationships.iter().any(|edge| {
            edge.relation == RelationshipType::BelongsTo
                && edge.from == vm_id
                && edge.to.as_ref() == Some(&vapp_id)
        }));
    }

    #[test]
    fn applies_provider_config_schema_default_and_namespaced_scope() {
        let mut mr = resource(
            "compute.vcd.m.carekube.com",
            "Machine",
            "node",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"tenant"}}}),
            Some("revision-vcd"),
        );
        mr.provider_config_default_kind = Some("ProviderConfig".into());
        mr.namespace = Some("crossplane-system".into());
        mr.original["metadata"]["namespace"] = json!("crossplane-system");
        let mut pc = resource(
            "vcd.m.carekube.com",
            "ProviderConfig",
            "tenant",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-vcd"),
        );
        pc.namespace = Some("crossplane-system".into());
        pc.original["metadata"]["namespace"] = json!("crossplane-system");
        let pc_id = pc.id();
        let graph = RelationshipGraph::build(&[mr, pc]);
        assert!(graph.relationships.iter().any(|edge| {
            edge.relation == RelationshipType::ManagedBy && edge.to.as_ref() == Some(&pc_id)
        }));
    }

    #[test]
    fn resolves_omitted_v1_kind_within_its_provider_api_model() {
        let mr = resource(
            "vapp.vcd.carekube.com",
            "VM",
            "legacy",
            ResourceFamily::ManagedResource,
            json!({"spec":{"providerConfigRef":{"name":"default"}}}),
            Some("revision-vcd"),
        );
        let v1 = resource(
            "vcd.carekube.com",
            "ProviderConfig",
            "default",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-vcd"),
        );
        let v2 = resource(
            "vcd.m.carekube.com",
            "ClusterProviderConfig",
            "default",
            ResourceFamily::ProviderConfig,
            json!({}),
            Some("revision-vcd"),
        );
        let v1_id = v1.id();
        let graph = RelationshipGraph::build(&[mr, v1, v2]);
        assert!(graph.relationships.iter().any(|edge| {
            edge.relation == RelationshipType::ManagedBy && edge.to.as_ref() == Some(&v1_id)
        }));
    }

    #[test]
    fn selector_edges_require_typed_identity_and_match_labels() {
        let source = resource(
            "compute.example.org",
            "Machine",
            "node",
            ResourceFamily::ManagedResource,
            json!({"spec":{"networkSelector":{"apiVersion":"net.example.org/v1","kind":"Network","matchLabels":{"env":"prod"}}}}),
            None,
        );
        let selected = resource(
            "net.example.org",
            "Network",
            "prod-net",
            ResourceFamily::Other,
            json!({"metadata":{"labels":{"env":"prod"}}}),
            None,
        );
        let other = resource(
            "net.example.org",
            "Network",
            "dev-net",
            ResourceFamily::Other,
            json!({"metadata":{"labels":{"env":"dev"}}}),
            None,
        );
        let graph = RelationshipGraph::build(&[source, selected, other]);
        let selected_edges: Vec<_> = graph
            .relationships
            .iter()
            .filter(|edge| edge.relation == RelationshipType::Selects && edge.to.is_some())
            .collect();
        assert_eq!(selected_edges.len(), 1);
        assert_eq!(selected_edges[0].to.as_ref().unwrap().name, "prod-net");
    }

    #[test]
    fn owner_relationships_and_cycles_are_retained_without_recursive_traversal() {
        let owner = resource(
            "xr.example.org",
            "Cluster",
            "root",
            ResourceFamily::CompositeResource,
            json!({}),
            None,
        );
        let child = resource(
            "infra.example.org",
            "Machine",
            "node",
            ResourceFamily::ManagedResource,
            json!({"metadata":{"ownerReferences":[{"apiVersion":"xr.example.org/v1","kind":"Cluster","name":"root"}]},"spec":{"peerRef":{"apiVersion":"infra.example.org/v1","kind":"Machine","name":"other"}}}),
            None,
        );
        let other = resource(
            "infra.example.org",
            "Machine",
            "other",
            ResourceFamily::ManagedResource,
            json!({"spec":{"peerRef":{"apiVersion":"infra.example.org/v1","kind":"Machine","name":"node"}}}),
            None,
        );
        let graph = RelationshipGraph::build(&[owner, child, other]);
        assert!(graph
            .relationships
            .iter()
            .any(|edge| edge.relation == RelationshipType::Owns
                && edge.from.name == "root"
                && edge.to.as_ref().is_some_and(|id| id.name == "node")));
        assert!(
            graph
                .relationships
                .iter()
                .filter(|edge| edge.relation == RelationshipType::References)
                .count()
                >= 2
        );
    }
}
