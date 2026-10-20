use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, Result};
use kube::{
    api::{Api, DynamicObject, ListParams},
    core::{ApiResource, GroupVersionKind},
    discovery::{pinned_group, verbs, ApiCapabilities, Scope},
    Client, Config,
};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::{
    crossplane::{self, CrdInfo},
    model::{sort_resources, Resource, ResourceFamily},
};

type DiscoveredVersion = std::result::Result<Vec<(ApiResource, ApiCapabilities)>, &'static str>;

#[derive(Clone)]
pub struct Kubernetes {
    client: Client,
    pub context: String,
}

impl Kubernetes {
    pub async fn connect() -> Result<Self> {
        let config = Config::infer().await.context(
            "Could not load Kubernetes configuration. Check KUBECONFIG and the current context.",
        )?;
        let context = context_label();
        Ok(Self {
            client: Client::try_from(config).context("Could not initialize Kubernetes client")?,
            context,
        })
    }

    pub async fn list_managed_resources(&self) -> Result<(Vec<Resource>, Vec<String>)> {
        info!(context = %self.context, "starting Crossplane resource discovery");
        let crds: Api<DynamicObject> = Api::all_with(
            self.client.clone(),
            &ApiResource::from_gvk_with_plural(
                &GroupVersionKind::gvk("apiextensions.k8s.io", "v1", "CustomResourceDefinition"),
                "customresourcedefinitions",
            ),
        );
        let mut crd_items = Vec::new();
        let mut crd_params = ListParams {
            limit: Some(500),
            ..ListParams::default()
        };
        loop {
            let page = match crds.list(&crd_params).await {
                Ok(page) => page,
                Err(error) => {
                    tracing::error!(
                        context = %self.context,
                        error_kind = kube_error_kind(&error),
                        "Could not list CustomResourceDefinitions"
                    );
                    return Err(error).context(
                        "Could not discover CustomResourceDefinitions; check CRD list permissions",
                    );
                }
            };
            let next = non_empty_continue_token(page.metadata.continue_.clone());
            crd_items.extend(page.items);
            let Some(token) = next else { break };
            crd_params.continue_token = Some(token);
        }
        let crd_count = crd_items.len();
        let mut resources = Vec::new();
        let mut warnings = Vec::new();
        let mut confirmed_types = 0usize;
        let mut possible_types = 0usize;
        let mut excluded_types = 0usize;
        let mut listed_instances = 0usize;
        let mut discovered_versions: HashMap<String, DiscoveredVersion> = HashMap::new();
        info!(context = %self.context, crd_count, "loaded CRD definitions");
        for crd_object in crd_items {
            let crd_value = match serde_json::to_value(crd_object) {
                Ok(value) => value,
                Err(_) => {
                    excluded_types += 1;
                    warn!(context = %self.context, "could not decode a CRD definition");
                    continue;
                }
            };
            let Some(mut info) = parse_crd(&crd_value) else {
                excluded_types += 1;
                debug!(context = %self.context, "excluded CRD with incomplete discovery metadata");
                continue;
            };
            let classification = crossplane::classify_crd(&info);
            let reasons = crossplane::classification_reasons(&info);
            match classification {
                crate::model::Classification::ConfirmedManagedResource => {
                    confirmed_types += 1;
                    debug!(group = %info.group, kind = %info.kind, reasons = ?reasons, "confirmed Managed Resource CRD");
                }
                crate::model::Classification::PossibleManagedResource => {
                    possible_types += 1;
                    info!(group = %info.group, kind = %info.kind, reasons = ?reasons, "possible Managed Resource CRD retained");
                }
                crate::model::Classification::NotManagedResource => {
                    excluded_types += 1;
                    continue;
                }
            }
            if info.family != ResourceFamily::ManagedResource {
                excluded_types += 1;
                continue;
            }
            let version = info.api_version.rsplit('/').next().unwrap_or("v1");
            if !discovered_versions.contains_key(&info.api_version) {
                let group_version = kube::core::GroupVersion::gv(&info.group, version);
                let resources = pinned_group(&self.client, &group_version)
                    .await
                    .map(|group| group.versioned_resources(version))
                    .map_err(|error| kube_error_kind(&error));
                discovered_versions.insert(info.api_version.clone(), resources);
            }
            let version_resources = match discovered_versions
                .get(&info.api_version)
                .expect("group/version discovery cache entry was inserted")
            {
                Ok(resources) => resources,
                Err(error_kind) => {
                    warn!(group = %info.group, version, kind = %info.kind, plural = %info.plural, error_kind, "Kubernetes API group/version discovery failed");
                    warnings.push(format!(
                        "{} ({}): API discovery failed ({error_kind})",
                        info.kind, info.api_version
                    ));
                    continue;
                }
            };
            let Some((api_resource, capabilities)) = version_resources
                .iter()
                .find(|(resource, caps)| discovery_resource_matches(&info, resource, caps))
                .cloned()
            else {
                warn!(group = %info.group, version, kind = %info.kind, plural = %info.plural, "API discovery has no LIST resource matching the CRD");
                warnings.push(format!(
                    "{} ({}): API version/resource not found in discovery",
                    info.kind, info.api_version
                ));
                continue;
            };
            let crd_namespaced = info.namespaced;
            let discovered_namespaced = matches!(capabilities.scope, Scope::Namespaced);
            if crd_namespaced != discovered_namespaced {
                warn!(
                    group = %info.group,
                    version,
                    kind = %info.kind,
                    crd_scope = if crd_namespaced { "Namespaced" } else { "Cluster" },
                    discovery_scope = if discovered_namespaced { "Namespaced" } else { "Cluster" },
                    "CRD and API discovery scopes differ; using API discovery scope"
                );
            }
            info.namespaced = discovered_namespaced;
            debug!(
                context = %self.context,
                group = %info.group,
                version,
                kind = %info.kind,
                plural = %info.plural,
                scope = if info.namespaced { "Namespaced" } else { "Cluster" },
                "resolved dynamic API resource"
            );
            let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &api_resource);
            let mut params = ListParams {
                limit: Some(500),
                ..ListParams::default()
            };
            let mut list_error = None;
            let mut type_count = 0usize;
            loop {
                match api.list(&params).await {
                    Ok(list) => {
                        let next = non_empty_continue_token(list.metadata.continue_.clone());
                        let mut page_values = Vec::new();
                        for object in list.items {
                            let value = match serde_json::to_value(object) {
                                Ok(value) => value,
                                Err(_) => {
                                    warn!(group = %info.group, version, kind = %info.kind, "could not decode a listed resource object");
                                    continue;
                                }
                            };
                            page_values.push(value);
                        }
                        let page_resources = crossplane::normalize_objects(page_values, &info);
                        type_count += page_resources.len();
                        listed_instances += page_resources.len();
                        let possible_resources: Vec<_> = page_resources
                            .iter()
                            .filter(|resource| {
                                resource.classification
                                    == crate::model::Classification::PossibleManagedResource
                            })
                            .collect();
                        if !possible_resources.is_empty() {
                            let reasons: BTreeSet<_> = possible_resources
                                .iter()
                                .flat_map(|resource| {
                                    crossplane::object_classification_reasons(
                                        &info,
                                        &resource.original,
                                    )
                                })
                                .collect();
                            info!(
                                group = %info.group,
                                version,
                                kind = %info.kind,
                                possible_instance_count = possible_resources.len(),
                                reasons = ?reasons,
                                "listed resources remain possible Managed Resources"
                            );
                        }
                        resources.extend(page_resources);
                        let Some(token) = next else { break };
                        params.continue_token = Some(token);
                    }
                    Err(error) => {
                        list_error = Some(kube_error_kind(&error));
                        break;
                    }
                }
            }
            if let Some(error) = list_error {
                warn!(kind = %info.kind, group = %info.group, version, plural = %info.plural, error_kind = error, "Could not list a discovered API resource");
                let scope = if info.namespaced {
                    "namespaced"
                } else {
                    "cluster-scoped"
                };
                warnings.push(format!(
                    "{} ({scope}): Kubernetes API list failed ({error})",
                    info.kind
                ));
            } else {
                info!(
                    context = %self.context,
                    group = %info.group,
                    version,
                    kind = %info.kind,
                    plural = %info.plural,
                    scope = if info.namespaced { "Namespaced" } else { "Cluster" },
                    instance_count = type_count,
                    "listed Managed Resource instances"
                );
            }
        }
        sort_resources(&mut resources);
        info!(
            context = %self.context,
            crd_count,
            confirmed_managed_resource_types = confirmed_types,
            possible_managed_resource_types = possible_types,
            excluded_types,
            successfully_listed_instances = listed_instances,
            normalized_resources = resources.len(),
            "Crossplane resource discovery completed"
        );
        Ok((resources, warnings))
    }
}

fn kube_error_kind(error: &kube::Error) -> &'static str {
    match error {
        kube::Error::Api(response) => match response.code {
            401 => "unauthorized",
            403 => "forbidden",
            404 => "not_found",
            429 => "rate_limited",
            code if code >= 500 => "server_error",
            _ => "api_error",
        },
        kube::Error::Discovery(_) => "discovery_error",
        kube::Error::HyperError(_) => "transport_error",
        _ => "kubernetes_request_error",
    }
}

fn non_empty_continue_token(token: Option<String>) -> Option<String> {
    token.filter(|token| !token.is_empty())
}

fn discovery_resource_matches(
    crd: &CrdInfo,
    resource: &ApiResource,
    capabilities: &ApiCapabilities,
) -> bool {
    let version = crd.api_version.rsplit('/').next().unwrap_or_default();
    resource.group == crd.group
        && resource.version == version
        && resource.kind == crd.kind
        && resource.plural == crd.plural
        && capabilities.supports_operation(verbs::LIST)
}

pub async fn kube_context() -> Result<String> {
    Config::infer().await.context(
        "Could not load Kubernetes configuration. Check KUBECONFIG and the current context.",
    )?;
    Ok(context_label())
}

fn context_label() -> String {
    kube::config::Kubeconfig::read()
        .ok()
        .and_then(|config| config.current_context)
        .or_else(|| std::env::var_os("KUBERNETES_SERVICE_HOST").map(|_| "in-cluster".to_owned()))
        .unwrap_or_else(|| "unknown context".to_owned())
}

pub(crate) fn parse_crd(value: &Value) -> Option<CrdInfo> {
    let metadata = value.get("metadata")?;
    let spec = value.get("spec")?;
    let group = spec.get("group")?.as_str()?.to_owned();
    let names = spec.get("names")?;
    let kind = names.get("kind")?.as_str()?.to_owned();
    let plural = names.get("plural")?.as_str()?.to_owned();
    let version = spec
        .get("versions")?
        .as_array()?
        .iter()
        .find(|version| version.get("served").and_then(Value::as_bool) == Some(true))?;
    let version_name = version.get("name")?.as_str()?;
    let schema = version
        .pointer("/schema/openAPIV3Schema")
        .cloned()
        .unwrap_or(Value::Null);
    let mut crd = CrdInfo {
        group: group.clone(),
        kind,
        namespaced: spec.get("scope").and_then(Value::as_str) == Some("Namespaced"),
        schema,
        labels: metadata.get("labels").cloned().unwrap_or(Value::Null),
        annotations: metadata.get("annotations").cloned().unwrap_or(Value::Null),
        owner_references: metadata
            .get("ownerReferences")
            .cloned()
            .unwrap_or(Value::Null),
        categories: names
            .get("categories")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        family: ResourceFamily::Other,
        api_version: format!("{group}/{version_name}"),
        plural,
    };
    crd.family = crossplane::family_for_crd(&crd);
    Some(crd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_discovered_crd_scope_and_served_version() {
        let value = json!({"metadata":{"name":"buckets.s3.aws.m.upbound.io"},"spec":{"group":"s3.aws.m.upbound.io","scope":"Namespaced","names":{"kind":"Bucket","plural":"buckets"},"versions":[{"name":"v1beta1","served":true,"schema":{"openAPIV3Schema":{"properties":{"spec":{"properties":{"forProvider":{}}},"status":{"properties":{"atProvider":{}}}}}}}]}});
        let crd = parse_crd(&value).unwrap();
        assert!(crd.namespaced);
        assert_eq!(crd.api_version, "s3.aws.m.upbound.io/v1beta1");
        assert_eq!(crd.family, ResourceFamily::ManagedResource);
    }

    #[test]
    fn matches_discovered_api_by_group_version_kind_plural_and_list_verb() {
        use kube::{core::GroupVersionKind, discovery::ApiResource};

        let value: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm-crd.json"))
                .unwrap();
        let crd = parse_crd(&value).unwrap();
        let gvk = GroupVersionKind::gvk("vapp.vcd.carekube.com", "v1alpha1", "VM");
        let matching = ApiResource::from_gvk_with_plural(&gvk, "vms");
        let capabilities = ApiCapabilities {
            scope: Scope::Cluster,
            subresources: vec![],
            operations: vec![verbs::LIST.to_owned()],
        };
        assert!(discovery_resource_matches(&crd, &matching, &capabilities));

        let wrong_plural = ApiResource::from_gvk_with_plural(&gvk, "virtualmachines");
        assert!(!discovery_resource_matches(
            &crd,
            &wrong_plural,
            &capabilities
        ));
        let no_list = ApiCapabilities {
            operations: vec![verbs::GET.to_owned()],
            ..capabilities
        };
        assert!(!discovery_resource_matches(&crd, &matching, &no_list));
    }

    #[test]
    fn empty_list_continue_token_ends_pagination() {
        assert_eq!(non_empty_continue_token(None), None);
        assert_eq!(non_empty_continue_token(Some(String::new())), None);
        assert_eq!(
            non_empty_continue_token(Some("next-page".to_owned())),
            Some("next-page".to_owned())
        );
    }
}
