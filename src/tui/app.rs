use std::collections::HashSet;

use crate::core::{
    models::{Resource, ResourceFamily, ResourceId},
    relationships::{RelationshipGraph, RelationshipType},
};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Providers,
    Resources,
    System,
    Details,
    Yaml,
    Logs,
    Events,
    Describe,
    Help,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub view: View,
    pub selected: usize,
    pub related_selected: usize,
    pub provider: Option<ResourceId>,
    pub unassigned_only: bool,
    pub unresolved_only: bool,
    pub filter: String,
    pub tree: bool,
    pub collapsed: HashSet<ResourceId>,
    pub health_only: bool,
    pub detail_scroll: u16,
}

pub struct App {
    pub resources: Vec<Resource>,
    pub graph: RelationshipGraph,
    pub providers: Vec<ResourceId>,
    pub selected: usize,
    pub related_selected: usize,
    pub selected_provider: Option<ResourceId>,
    pub unassigned_only: bool,
    pub unresolved_only: bool,
    pub filter: String,
    pub input: String,
    pub input_mode: InputMode,
    pub view: View,
    pub tree: bool,
    pub collapsed: HashSet<ResourceId>,
    pub health_only: bool,
    pub detail_scroll: u16,
    pub history: Vec<HistoryEntry>,
    pub loading: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub events: Vec<Value>,
    pub context: String,
    pub cluster: Option<String>,
    pub user: Option<String>,
    pub kubernetes_version: Option<String>,
    pub crossplane_version: Option<String>,
    pub crossplane_version_diagnostic: Option<String>,
    pub provider_images: std::collections::HashMap<ResourceId, String>,
    pub provider_identities:
        std::collections::HashMap<ResourceId, crate::core::kubernetes::ProviderIdentity>,
    pub yaml_object: Option<Value>,
    pub yaml_revealed: bool,
    pub viewer_search: String,
    pub logs: Vec<String>,
    pub log_provider: Option<String>,
    pub log_pods: Vec<crate::core::kubernetes::ProviderPod>,
    pub log_pod_selected: usize,
    pub log_following: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    None,
    Filter,
    Command,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VersionCache {
    pub kubernetes: Option<String>,
    pub crossplane: Option<String>,
    pub crossplane_diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderEntry {
    Config(ResourceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeRow {
    pub index: usize,
    pub depth: usize,
    pub last: bool,
    pub has_children: bool,
}

impl App {
    pub fn new(context: String) -> Self {
        Self {
            resources: Vec::new(),
            graph: RelationshipGraph::default(),
            providers: Vec::new(),
            selected: 0,
            related_selected: 0,
            selected_provider: None,
            unassigned_only: false,
            unresolved_only: false,
            filter: String::new(),
            input: String::new(),
            input_mode: InputMode::None,
            view: View::Providers,
            tree: true,
            collapsed: HashSet::new(),
            health_only: false,
            detail_scroll: 0,
            history: Vec::new(),
            loading: true,
            error: None,
            warnings: Vec::new(),
            events: Vec::new(),
            context,
            cluster: None,
            user: None,
            kubernetes_version: None,
            crossplane_version: None,
            crossplane_version_diagnostic: None,
            provider_images: std::collections::HashMap::new(),
            provider_identities: std::collections::HashMap::new(),
            yaml_object: None,
            yaml_revealed: false,
            viewer_search: String::new(),
            logs: Vec::new(),
            log_provider: None,
            log_pods: Vec::new(),
            log_pod_selected: 0,
            log_following: true,
        }
    }

    pub fn cache_versions(&mut self, versions: Option<VersionCache>) {
        if let Some(versions) = versions {
            self.kubernetes_version = versions.kubernetes;
            self.crossplane_version = versions.crossplane;
            self.crossplane_version_diagnostic = versions.crossplane_diagnostic;
        }
    }

    pub fn prepare_context_switch(&mut self, context: &str) {
        if self.context != context {
            self.kubernetes_version = None;
            self.crossplane_version = None;
            self.crossplane_version_diagnostic = None;
            self.provider_images.clear();
        }
    }

    pub fn replace_resources(
        &mut self,
        resources: Vec<Resource>,
        graph: RelationshipGraph,
        warnings: Vec<String>,
    ) {
        let old_selected = (self.view != View::Providers)
            .then(|| self.selected_resource().map(Resource::id))
            .flatten();
        let old_provider_selected = (self.view == View::Providers)
            .then(|| self.visible_provider_entries().get(self.selected).cloned())
            .flatten();
        self.resources = resources;
        self.graph = graph;
        self.providers = self
            .resources
            .iter()
            .filter(|resource| resource.family == ResourceFamily::ProviderConfig)
            .map(Resource::id)
            .collect();
        self.providers.sort_by(|left, right| {
            (&left.group, &left.kind, &left.namespace, &left.name).cmp(&(
                &right.group,
                &right.kind,
                &right.namespace,
                &right.name,
            ))
        });
        if self
            .selected_provider
            .as_ref()
            .is_some_and(|provider| !self.providers.contains(provider))
        {
            self.selected_provider = None;
            self.unassigned_only = true;
        }
        self.warnings = warnings;
        if let Some(diagnostic) = &self.crossplane_version_diagnostic {
            self.warnings.push(diagnostic.clone());
        }
        if self.view == View::Providers {
            if let Some(old_provider_selected) = old_provider_selected {
                self.selected = self
                    .visible_provider_entries()
                    .iter()
                    .position(|entry| entry == &old_provider_selected)
                    .unwrap_or(0);
            } else {
                self.selected = self.selected.min(self.current_len().saturating_sub(1));
            }
        } else if let Some(old_selected) = old_selected {
            let indices = self.visible_indices();
            if let Some(position) = indices
                .iter()
                .position(|index| self.resources[*index].id() == old_selected)
            {
                self.selected = position;
            } else {
                self.view = View::Resources;
                self.selected = self.selected.min(indices.len().saturating_sub(1));
            }
        } else {
            self.selected = self.selected.min(self.current_len().saturating_sub(1));
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        self.visible_tree_rows()
            .into_iter()
            .map(|row| row.index)
            .collect()
    }

    pub fn visible_tree_rows(&self) -> Vec<TreeRow> {
        let needle = self.filter.to_lowercase();
        let mut included: HashSet<ResourceId> = self
            .resources
            .iter()
            .filter(|resource| {
                let system_view = self.view == View::System;
                if system_view != resource.family.is_system()
                    || (!system_view && resource.family == ResourceFamily::ProviderConfig)
                {
                    return false;
                }
                let is_managed = resource.family == ResourceFamily::ManagedResource;
                let edges = self.graph.relationships.iter().filter(|edge| {
                    edge.relation == RelationshipType::ManagedBy && edge.from == resource.id()
                });
                let belongs = if system_view {
                    true
                } else if let Some(provider) = &self.selected_provider {
                    is_managed && edges.clone().any(|edge| edge.to.as_ref() == Some(provider))
                } else if self.unresolved_only {
                    is_managed && edges.clone().any(|edge| edge.to.is_none())
                } else if self.unassigned_only {
                    is_managed
                        && resource
                            .original
                            .pointer("/spec/providerConfigRef")
                            .is_none()
                } else {
                    true
                };
                let unhealthy = matches!(
                    resource.ready.status,
                    crate::core::ConditionStatus::False | crate::core::ConditionStatus::Unknown
                ) || matches!(
                    resource.synced.status,
                    crate::core::ConditionStatus::False | crate::core::ConditionStatus::Unknown
                );
                belongs
                    && (!self.health_only || unhealthy)
                    && (needle.is_empty()
                        || resource.name.to_lowercase().contains(&needle)
                        || resource.kind.to_lowercase().contains(&needle)
                        || resource.group.to_lowercase().contains(&needle)
                        || resource
                            .namespace
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&needle)
                        || resource
                            .ready
                            .status
                            .label()
                            .to_lowercase()
                            .contains(&needle))
            })
            .map(Resource::id)
            .collect();

        let mut edges = self.tree_edges();
        if self.tree {
            let mut changed = true;
            while changed {
                changed = false;
                for (parent, child) in &edges {
                    if included.contains(child) && included.insert(parent.clone()) {
                        changed = true;
                    }
                }
            }
        }
        edges.retain(|(parent, child)| included.contains(parent) && included.contains(child));
        let mut rows = Vec::new();
        if !self.tree {
            return self
                .resources
                .iter()
                .enumerate()
                .filter_map(|(index, resource)| {
                    included.contains(&resource.id()).then_some(TreeRow {
                        index,
                        depth: 0,
                        last: true,
                        has_children: false,
                    })
                })
                .collect();
        }
        let roots: Vec<_> = self
            .resources
            .iter()
            .enumerate()
            .filter(|(_, resource)| {
                let id = resource.id();
                included.contains(&id) && !edges.iter().any(|(_, child)| child == &id)
            })
            .map(|(index, resource)| (index, resource.id()))
            .collect();
        for (index, id) in roots {
            self.append_tree(
                &id,
                TreeRow {
                    index,
                    depth: 0,
                    last: true,
                    has_children: false,
                },
                &included,
                &edges,
                &mut HashSet::new(),
                &mut rows,
            );
        }
        // Render cyclic components once after rooted components.
        for (index, resource) in self.resources.iter().enumerate() {
            let id = resource.id();
            if included.contains(&id) && !rows.iter().any(|row| row.index == index) {
                self.append_tree(
                    &id,
                    TreeRow {
                        index,
                        depth: 0,
                        last: true,
                        has_children: false,
                    },
                    &included,
                    &edges,
                    &mut HashSet::new(),
                    &mut rows,
                );
            }
        }
        rows
    }

    fn tree_edges(&self) -> Vec<(ResourceId, ResourceId)> {
        self.graph
            .relationships
            .iter()
            .filter_map(|edge| match edge.relation {
                RelationshipType::Composes | RelationshipType::Owns => {
                    edge.to.clone().map(|child| (edge.from.clone(), child))
                }
                RelationshipType::BelongsTo => {
                    edge.to.clone().map(|parent| (parent, edge.from.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn append_tree(
        &self,
        id: &ResourceId,
        row: TreeRow,
        included: &HashSet<ResourceId>,
        edges: &[(ResourceId, ResourceId)],
        path: &mut HashSet<ResourceId>,
        rows: &mut Vec<TreeRow>,
    ) {
        if !path.insert(id.clone()) {
            return;
        }
        let mut children: Vec<_> = edges
            .iter()
            .filter(|(parent, child)| parent == id && included.contains(child))
            .map(|(_, child)| child.clone())
            .collect();
        children.sort_by(|a, b| a.name.cmp(&b.name));
        rows.push(TreeRow {
            has_children: !children.is_empty(),
            ..row
        });
        if !self.collapsed.contains(id) || !self.filter.is_empty() {
            for (position, child) in children.iter().enumerate() {
                if let Some(child_index) = self
                    .resources
                    .iter()
                    .position(|resource| resource.id() == *child)
                {
                    self.append_tree(
                        child,
                        TreeRow {
                            index: child_index,
                            depth: row.depth + 1,
                            last: position + 1 == children.len(),
                            has_children: false,
                        },
                        included,
                        edges,
                        path,
                        rows,
                    );
                }
            }
        }
        path.remove(id);
    }

    pub fn selected_resource(&self) -> Option<&Resource> {
        self.visible_indices()
            .get(self.selected)
            .and_then(|index| self.resources.get(*index))
    }

    pub fn selected_provider_config(&self) -> Option<&Resource> {
        let entries = self.visible_provider_entries();
        let ProviderEntry::Config(id) = entries.get(self.selected)?;
        self.resources.iter().find(|resource| resource.id() == *id)
    }

    pub fn related_resource_ids(&self) -> Vec<ResourceId> {
        let Some(resource) = self.selected_resource() else {
            return Vec::new();
        };
        let id = resource.id();
        let mut ids: Vec<_> = self
            .graph
            .relationships
            .iter()
            .filter_map(|edge| {
                if edge.from == id {
                    edge.to.clone()
                } else if edge.to.as_ref() == Some(&id) {
                    Some(edge.from.clone())
                } else {
                    None
                }
            })
            .collect();
        ids.sort_by(|left, right| {
            (&left.group, &left.kind, &left.namespace, &left.name).cmp(&(
                &right.group,
                &right.kind,
                &right.namespace,
                &right.name,
            ))
        });
        ids.dedup();
        ids
    }

    pub fn current_len(&self) -> usize {
        if self.view == View::Providers {
            self.visible_provider_entries().len()
        } else {
            self.visible_indices().len()
        }
    }

    pub fn visible_provider_entries(&self) -> Vec<ProviderEntry> {
        let needle = self.filter.to_lowercase();
        self.providers
            .iter()
            .filter_map(|id| {
                let matches = self
                    .resources
                    .iter()
                    .find(|resource| resource.id() == *id)
                    .is_some_and(|resource| {
                        resource.name.to_lowercase().contains(&needle)
                            || resource.kind.to_lowercase().contains(&needle)
                            || resource.group.to_lowercase().contains(&needle)
                            || resource
                                .namespace
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&needle)
                            || resource
                                .ready
                                .status
                                .label()
                                .to_lowercase()
                                .contains(&needle)
                    });
                matches.then(|| ProviderEntry::Config(id.clone()))
            })
            .collect()
    }

    pub fn provider_resource_count(&self, provider: &ResourceId) -> usize {
        let identities: HashSet<_> = self
            .resources
            .iter()
            .filter(|resource| resource.family == ResourceFamily::ManagedResource)
            .map(Resource::id)
            .collect();
        identities
            .into_iter()
            .filter(|id| {
                self.graph.relationships.iter().any(|edge| {
                    edge.relation == RelationshipType::ManagedBy
                        && edge.from == *id
                        && edge.to.as_ref() == Some(provider)
                })
            })
            .count()
    }

    pub fn move_selection(&mut self, amount: isize) {
        let len = self.current_len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as isize + amount).clamp(0, len as isize - 1) as usize;
    }

    pub fn push_history(&mut self) {
        self.history.push(HistoryEntry {
            view: self.view,
            selected: self.selected,
            related_selected: self.related_selected,
            provider: self.selected_provider.clone(),
            unassigned_only: self.unassigned_only,
            unresolved_only: self.unresolved_only,
            filter: self.filter.clone(),
            tree: self.tree,
            collapsed: self.collapsed.clone(),
            health_only: self.health_only,
            detail_scroll: self.detail_scroll,
        });
    }

    pub fn back(&mut self) {
        if let Some(entry) = self.history.pop() {
            self.view = entry.view;
            self.selected = entry.selected;
            self.related_selected = entry.related_selected;
            self.selected_provider = entry.provider;
            self.unassigned_only = entry.unassigned_only;
            self.unresolved_only = entry.unresolved_only;
            self.filter = entry.filter;
            self.tree = entry.tree;
            self.collapsed = entry.collapsed;
            self.health_only = entry.health_only;
            self.detail_scroll = entry.detail_scroll;
        } else if self.view != View::Providers {
            self.view = View::Providers;
            self.selected = 0;
            self.selected_provider = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        crossplane::{normalize, CrdInfo},
        models::sort_resources,
    };
    use serde_json::json;

    fn resource(name: &str) -> Resource {
        let crd = CrdInfo {
            group: "infra.example.org".into(),
            kind: "Machine".into(),
            namespaced: false,
            schema: json!({}),
            labels: json!({}),
            annotations: json!({}),
            owner_references: json!([]),
            categories: vec![],
            family: ResourceFamily::ManagedResource,
            api_version: "infra.example.org/v1".into(),
            plural: "machines".into(),
            provider_config_default_kind: None,
            vcd_vapp_name_reference: false,
        };
        normalize(
            json!({"apiVersion":"infra.example.org/v1","kind":"Machine","metadata":{"name":name}}),
            &crd,
        )
    }

    #[test]
    fn version_cache_survives_resource_refresh_and_clears_on_context_change() {
        let mut app = App::new("ctx-a".into());
        app.cache_versions(Some(VersionCache {
            kubernetes: Some("v1.35.0".into()),
            crossplane: Some("v2.1.0".into()),
            crossplane_diagnostic: Some("version discovery failed".into()),
        }));
        app.cache_versions(None);
        app.replace_resources(Vec::new(), RelationshipGraph::default(), vec![]);
        assert_eq!(app.kubernetes_version.as_deref(), Some("v1.35.0"));
        assert_eq!(app.crossplane_version.as_deref(), Some("v2.1.0"));
        assert_eq!(
            app.crossplane_version_diagnostic.as_deref(),
            Some("version discovery failed")
        );
        assert!(app
            .warnings
            .iter()
            .any(|warning| warning == "version discovery failed"));
        app.prepare_context_switch("ctx-a");
        assert_eq!(app.kubernetes_version.as_deref(), Some("v1.35.0"));
        app.prepare_context_switch("ctx-b");
        assert_eq!(app.kubernetes_version, None);
        assert_eq!(app.crossplane_version, None);
        assert_eq!(app.crossplane_version_diagnostic, None);
        app.cache_versions(Some(VersionCache {
            kubernetes: None,
            crossplane: Some("v2.2.0".into()),
            crossplane_diagnostic: None,
        }));
        assert_eq!(app.kubernetes_version, None);
        assert_eq!(app.crossplane_version.as_deref(), Some("v2.2.0"));
    }

    #[test]
    fn filter_and_selection_survive_refresh_by_identity() {
        let mut app = App::new("test".into());
        app.view = View::Resources;
        let mut alpha = resource("alpha");
        alpha.family = ResourceFamily::ManagedResource;
        let mut beta = resource("beta");
        beta.family = ResourceFamily::ManagedResource;
        app.resources = vec![alpha, beta];
        app.selected = 1;
        let mut refreshed = vec![resource("beta"), resource("alpha"), resource("gamma")];
        sort_resources(&mut refreshed);
        app.replace_resources(refreshed, RelationshipGraph::default(), vec![]);
        assert_eq!(app.selected_resource().unwrap().name, "beta");
        app.filter = "GAMMA".into();
        assert_eq!(app.visible_indices().len(), 1);
        assert_eq!(app.resources[app.visible_indices()[0]].name, "gamma");
    }

    #[test]
    fn navigation_history_restores_view_filter_and_tree_mode() {
        let mut app = App::new("test".into());
        app.view = View::Resources;
        app.filter = "alpha".into();
        app.tree = false;
        app.push_history();
        app.view = View::Details;
        app.filter.clear();
        app.tree = true;
        app.back();
        assert_eq!(app.view, View::Resources);
        assert_eq!(app.filter, "alpha");
        assert!(!app.tree);
    }

    #[test]
    fn provider_selector_contains_only_actual_provider_configs() {
        let mut app = App::new("test".into());
        app.view = View::Providers;
        let mut namespaced = resource("default");
        namespaced.kind = "ProviderConfig".into();
        namespaced.family = ResourceFamily::ProviderConfig;
        namespaced.namespace = Some("team-a".into());
        let mut cluster = resource("default");
        cluster.kind = "ClusterProviderConfig".into();
        cluster.family = ResourceFamily::ProviderConfig;
        let mut same_name_other_group = namespaced.clone();
        same_name_other_group.group = "other.example.org".into();
        same_name_other_group.api_version = "other.example.org/v1".into();
        app.replace_resources(
            vec![
                resource("unassigned"),
                namespaced.clone(),
                cluster.clone(),
                same_name_other_group.clone(),
            ],
            RelationshipGraph::default(),
            vec![],
        );
        let entries = app.visible_provider_entries();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&ProviderEntry::Config(namespaced.id())));
        assert!(entries.contains(&ProviderEntry::Config(cluster.id())));
        assert!(entries.contains(&ProviderEntry::Config(same_name_other_group.id())));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry, ProviderEntry::Config(_)))
                .count(),
            3
        );

        app.view = View::Resources;
        app.unassigned_only = true;
        assert_eq!(app.visible_indices().len(), 1);
    }

    #[test]
    fn unresolved_resources_remain_visible_in_all_and_have_a_separate_filter() {
        let mut item = resource("orphan");
        item.family = ResourceFamily::ManagedResource;
        item.original["spec"]["providerConfigRef"] = json!({"name":"missing"});
        let graph = RelationshipGraph::build(std::slice::from_ref(&item));
        let mut app = App::new("test".into());
        app.view = View::Resources;
        app.resources = vec![item];
        app.graph = graph;
        assert_eq!(app.visible_indices().len(), 1);
        app.unresolved_only = true;
        assert_eq!(app.visible_indices().len(), 1);
        app.unresolved_only = false;
        app.unassigned_only = true;
        assert!(app.visible_indices().is_empty());
    }

    #[test]
    fn provider_counts_unique_instances_not_graph_edges() {
        let mut managed = resource("node");
        managed.family = ResourceFamily::ManagedResource;
        let mut config = resource("default");
        config.family = ResourceFamily::ProviderConfig;
        let managed_id = managed.id();
        let config_id = config.id();
        let edge = crate::core::relationships::Relationship {
            from: managed_id.clone(),
            to: Some(config_id.clone()),
            relation: RelationshipType::ManagedBy,
            unresolved: None,
        };
        let mut app = App::new("test".into());
        app.resources = vec![managed.clone(), managed, config];
        app.graph.relationships = vec![edge.clone(), edge];
        assert_eq!(app.provider_resource_count(&config_id), 1);
    }

    #[test]
    fn known_inverse_relationship_renders_as_tree_parent_and_child() {
        let mut parent = resource("seed");
        parent.kind = "Vapp".into();
        let mut child = resource("seed-cp01");
        child.kind = "VM".into();
        let parent_id = parent.id();
        let child_id = child.id();
        let mut app = App::new("test".into());
        app.view = View::Resources;
        app.resources = vec![child, parent];
        app.graph
            .relationships
            .push(crate::core::relationships::Relationship {
                from: child_id,
                to: Some(parent_id),
                relation: RelationshipType::BelongsTo,
                unresolved: None,
            });
        let rows = app.visible_tree_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(app.resources[rows[0].index].kind, "Vapp");
        assert!(rows[0].has_children);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(app.resources[rows[1].index].kind, "VM");
        assert!(rows[1].last);
    }

    #[test]
    fn system_objects_are_hidden_from_infrastructure_and_available_separately() {
        let mut definition = resource("bucket.mrd");
        definition.family = ResourceFamily::System;
        let mut revision = resource("provider-revision");
        revision.family = ResourceFamily::ProviderRevision;
        let mut app = App::new("test".into());
        app.view = View::Resources;
        app.resources = vec![definition, revision];
        assert!(app.visible_indices().is_empty());
        app.view = View::System;
        assert_eq!(app.visible_indices().len(), 2);
    }
}
