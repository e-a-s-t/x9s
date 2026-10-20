use crate::model::Resource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    List,
    Details,
}

pub struct App {
    pub resources: Vec<Resource>,
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
    pub view: View,
    pub loading: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub context: String,
}

impl App {
    pub fn visible_indices(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        self.resources
            .iter()
            .enumerate()
            .filter_map(|(index, resource)| {
                if resource.classification != crate::model::Classification::ConfirmedManagedResource
                {
                    return None;
                }
                let matches = needle.is_empty()
                    || resource.name.to_lowercase().contains(&needle)
                    || resource.kind.to_lowercase().contains(&needle)
                    || resource
                        .namespace
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&needle);
                matches.then_some(index)
            })
            .collect()
    }

    pub fn selected_resource(&self) -> Option<&Resource> {
        self.visible_indices()
            .get(self.selected)
            .and_then(|index| self.resources.get(*index))
    }

    pub fn replace_resources(&mut self, resources: Vec<Resource>, warnings: Vec<String>) {
        let old_identity = self.selected_resource().map(|r| {
            (
                r.api_version.clone(),
                r.kind.clone(),
                r.namespace.clone(),
                r.name.clone(),
            )
        });
        self.resources = resources;
        self.warnings = warnings;
        let visible = self.visible_indices();
        self.selected = old_identity
            .and_then(|identity| {
                visible.iter().position(|index| {
                    let r = &self.resources[*index];
                    (
                        r.api_version.clone(),
                        r.kind.clone(),
                        r.namespace.clone(),
                        r.name.clone(),
                    ) == identity
                })
            })
            .unwrap_or_else(|| self.selected.min(visible.len().saturating_sub(1)));
    }

    pub fn move_selection(&mut self, amount: isize) {
        let len = self.visible_indices().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as isize + amount).clamp(0, len as isize - 1) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crossplane::normalize,
        model::{sort_resources, ResourceFamily},
    };
    use serde_json::json;

    fn resource(name: &str, namespace: Option<&str>) -> Resource {
        let crd = crate::crossplane::CrdInfo {
            group: "s3.aws.m.upbound.io".into(),
            kind: "Bucket".into(),
            namespaced: namespace.is_some(),
            schema: json!({"properties":{"spec":{"properties":{"forProvider":{}}},"status":{"properties":{"atProvider":{}}}}}),
            labels: json!({}),
            annotations: json!({}),
            owner_references: json!([]),
            categories: vec![],
            family: ResourceFamily::ManagedResource,
            api_version: "s3.aws.m.upbound.io/v1".into(),
            plural: "buckets".into(),
        };
        let mut value = json!({"apiVersion":"s3.aws.m.upbound.io/v1","kind":"Bucket","metadata":{"name":name},"spec":{"forProvider":{}},"status":{"conditions":[{"type":"Ready","status":"True"}]}});
        if let Some(namespace) = namespace {
            value["metadata"]["namespace"] = json!(namespace);
        }
        normalize(value, &crd)
    }

    fn app() -> App {
        App {
            resources: vec![
                resource("alpha", Some("team-a")),
                resource("beta", Some("team-b")),
            ],
            selected: 0,
            filter: String::new(),
            filtering: false,
            view: View::List,
            loading: false,
            error: None,
            warnings: vec![],
            context: "test".into(),
        }
    }

    #[test]
    fn filtering_matches_name_kind_namespace_without_case() {
        let mut app = app();
        app.filter = "TEAM-B".into();
        assert_eq!(app.visible_indices(), vec![1]);
        app.filter = "buc".into();
        assert_eq!(app.visible_indices(), vec![0, 1]);
    }

    #[test]
    fn sorting_and_selection_preservation_are_stable() {
        let mut app = app();
        app.selected = 1;
        let selected_name = app.selected_resource().unwrap().name.clone();
        let mut reordered = vec![
            resource("beta", Some("team-b")),
            resource("aardvark", Some("team-a")),
            resource("alpha", Some("team-a")),
        ];
        sort_resources(&mut reordered);
        assert_eq!(
            reordered
                .iter()
                .map(|resource| resource.name.as_str())
                .collect::<Vec<_>>(),
            vec!["aardvark", "alpha", "beta"]
        );
        app.replace_resources(reordered, vec![]);
        assert_eq!(app.selected_resource().unwrap().name, selected_name);
        assert_eq!(app.selected, 2);
        app.move_selection(99);
        assert_eq!(app.selected, 2);
    }
}
