use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use anyhow::{Context, Result};
use kube::{
    api::{Api, DynamicObject, ListParams},
    core::{ApiResource, GroupVersionKind},
    discovery::{pinned_group, verbs, ApiCapabilities, Scope},
    Client, Config,
};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::core::{
    crossplane::{self, CrdInfo},
    models::{sort_resources, Classification, Resource, ResourceFamily},
    relationships::{self, RelationshipGraph, TargetReference},
};

type DiscoveredVersion = std::result::Result<Vec<(ApiResource, ApiCapabilities)>, &'static str>;

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryError {
    pub code: String,
    pub message: String,
    pub group: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TypeClassification {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub classification: crate::core::models::Classification,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DiscoveryReport {
    pub context: String,
    pub connected: bool,
    pub crd_count: usize,
    pub confirmed_types: usize,
    pub possible_types: usize,
    pub excluded_types: usize,
    pub managed_instance_count: usize,
    pub resources: Vec<Resource>,
    /// Infrastructure instances, ProviderConfigs, and diagnostic system objects.
    pub all_resources: Vec<Resource>,
    pub graph: RelationshipGraph,
    pub classifications: Vec<TypeClassification>,
    pub errors: Vec<DiscoveryError>,
    pub relationship_errors: Vec<DiscoveryError>,
    /// Installed package references resolved for ProviderConfigs through owner UIDs.
    pub provider_images: HashMap<crate::core::ResourceId, String>,
    pub provider_package_diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CrossplaneVersionDiscovery {
    pub version: Option<String>,
    pub diagnostic: Option<String>,
}

impl DiscoveryReport {
    pub fn is_partial(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Clone)]
pub struct Kubernetes {
    client: Client,
    pub context: String,
    discovery_cache: Arc<tokio::sync::Mutex<HashMap<String, DiscoveredVersion>>>,
}

impl Kubernetes {
    pub async fn connect(context: Option<&str>) -> Result<Self> {
        let config = match context {
            Some(context) => Config::from_kubeconfig(&kube::config::KubeConfigOptions {
                context: Some(context.to_owned()),
                ..Default::default()
            })
            .await
            .map_err(anyhow::Error::from),
            None => Config::infer().await.map_err(anyhow::Error::from),
        }
        .context(
            "Could not load Kubernetes configuration. Check KUBECONFIG and the selected context.",
        )?;
        let context = context.map(str::to_owned).unwrap_or_else(context_label);
        Ok(Self {
            client: Client::try_from(config).context("Could not initialize Kubernetes client")?,
            context,
            discovery_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    pub async fn api_server_version(&self) -> Option<String> {
        self.client
            .apiserver_version()
            .await
            .ok()
            .map(|version| version.git_version)
            .filter(|version| !version.is_empty())
    }

    pub async fn crossplane_version(&self) -> CrossplaneVersionDiscovery {
        let deployments: Api<DynamicObject> = Api::all_with(
            self.client.clone(),
            &ApiResource::from_gvk_with_plural(
                &GroupVersionKind::gvk("apps", "v1", "Deployment"),
                "deployments",
            ),
        );
        let items = match deployments.list(&ListParams::default()).await {
            Ok(list) => list.items,
            Err(error) => {
                let kind = kube_error_kind(&error);
                warn!(
                    error_kind = kind,
                    "Crossplane controller Deployment discovery failed"
                );
                return failed_crossplane_version_discovery(kind);
            }
        };
        let values: Vec<_> = items
            .into_iter()
            .map(|deployment| deployment.data)
            .collect();
        resolve_crossplane_version(&values)
    }

    pub async fn list_managed_resources(&self) -> Result<DiscoveryReport> {
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
        let mut all_resources = Vec::new();
        let mut errors = Vec::new();
        let mut classifications = Vec::new();
        let mut confirmed_types = 0usize;
        let mut possible_types = 0usize;
        let mut excluded_types = 0usize;
        let mut listed_instances = 0usize;
        let mut discovered_versions = self.discovery_cache.lock().await.clone();
        discovered_versions.retain(|_, result| result.is_ok());
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
            classifications.push(TypeClassification {
                group: info.group.clone(),
                version: info
                    .api_version
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                kind: info.kind.clone(),
                classification: classification.clone(),
                reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
            });
            match classification {
                Classification::ConfirmedManagedResource => {
                    confirmed_types += 1;
                    debug!(group = %info.group, kind = %info.kind, reasons = ?reasons, "confirmed Managed Resource CRD");
                }
                Classification::PossibleManagedResource => {
                    possible_types += 1;
                    info!(group = %info.group, kind = %info.kind, reasons = ?reasons, "possible Managed Resource CRD retained");
                }
                Classification::NotManagedResource => {
                    excluded_types += 1;
                }
            }
            // Definitions are metadata, and unrelated APIs must not be listed as
            // infrastructure. Their exact objects may still be fetched below when
            // an infrastructure object authoritatively references them.
            if !matches!(
                info.family,
                ResourceFamily::ManagedResource
                    | ResourceFamily::CompositeResource
                    | ResourceFamily::ProviderConfig
            ) && !info.family.is_system()
            {
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
                    errors.push(DiscoveryError {
                        code: (*error_kind).to_owned(),
                        message: format!("API discovery failed for {}", info.kind),
                        group: Some(info.group.clone()),
                        version: Some(version.to_owned()),
                        kind: Some(info.kind.clone()),
                    });
                    continue;
                }
            };
            let Some((api_resource, capabilities)) = version_resources
                .iter()
                .find(|(resource, caps)| discovery_resource_matches(&info, resource, caps))
                .cloned()
            else {
                warn!(group = %info.group, version, kind = %info.kind, plural = %info.plural, "API discovery has no LIST resource matching the CRD");
                errors.push(DiscoveryError {
                    code: "resource_not_discovered".into(),
                    message: format!("No LIST-capable API resource matched {}", info.kind),
                    group: Some(info.group.clone()),
                    version: Some(version.to_owned()),
                    kind: Some(info.kind.clone()),
                });
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
                        if matches!(
                            info.family,
                            ResourceFamily::ManagedResource | ResourceFamily::CompositeResource
                        ) {
                            listed_instances += page_resources.len();
                        }
                        let possible_resources: Vec<_> = page_resources
                            .iter()
                            .filter(|resource| {
                                resource.classification == Classification::PossibleManagedResource
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
                        all_resources.extend(page_resources.iter().cloned());
                        resources.extend(page_resources.into_iter().filter(|resource| {
                            resource.family == ResourceFamily::ManagedResource
                                && resource.classification
                                    == Classification::ConfirmedManagedResource
                        }));
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
                errors.push(DiscoveryError {
                    code: error.to_owned(),
                    message: format!("Kubernetes API list failed for {} ({scope})", info.kind),
                    group: Some(info.group.clone()),
                    version: Some(version.to_owned()),
                    kind: Some(info.kind.clone()),
                });
            } else {
                info!(
                    context = %self.context,
                    group = %info.group,
                    version,
                    kind = %info.kind,
                    plural = %info.plural,
                    scope = if info.namespaced { "Namespaced" } else { "Cluster" },
                    instance_count = type_count,
                    family = info.family.label(),
                    "listed CRD instances"
                );
            }
        }
        let relationship_sources: Vec<_> = all_resources
            .iter()
            .filter(|resource| {
                matches!(
                    resource.family,
                    ResourceFamily::ManagedResource | ResourceFamily::CompositeResource
                )
            })
            .cloned()
            .collect();
        let (referenced_resources, reference_errors) = self
            .fetch_authoritative_targets(&relationship_sources, &mut discovered_versions)
            .await;
        all_resources.extend(referenced_resources);
        sort_resources(&mut resources);
        sort_resources(&mut all_resources);
        let mut resource_identities = std::collections::HashSet::new();
        resources.retain(|resource| resource_identities.insert(resource.id()));
        let mut graph_identities = std::collections::HashSet::new();
        all_resources.retain(|resource| graph_identities.insert(resource.id()));
        let graph = RelationshipGraph::build(&all_resources);
        let (provider_images, provider_package_diagnostics) =
            self.discover_provider_packages(&all_resources).await;
        for provider_config in all_resources
            .iter()
            .filter(|resource| resource.family == ResourceFamily::ProviderConfig)
        {
            let provider_id = provider_config.id();
            let associated: std::collections::HashSet<_> = all_resources
                .iter()
                .filter(|resource| resource.family == ResourceFamily::ManagedResource)
                .filter(|resource| {
                    graph.relationships.iter().any(|edge| {
                        edge.relation == relationships::RelationshipType::ManagedBy
                            && edge.from == resource.id()
                            && edge.to.as_ref() == Some(&provider_id)
                    })
                })
                .map(Resource::id)
                .collect();
            info!(
                provider_group = %provider_config.group,
                provider_kind = %provider_config.kind,
                provider_namespace = provider_config.namespace.as_deref().unwrap_or(""),
                provider_name = %provider_config.name,
                unique_managed_resource_count = associated.len(),
                "resolved ProviderConfig resource association summary"
            );
        }
        let resolved_schema_relationships = graph
            .relationships
            .iter()
            .filter(|edge| {
                edge.relation == relationships::RelationshipType::BelongsTo && edge.to.is_some()
            })
            .count();
        info!(
            resolved_schema_relationships,
            "resolved provider-schema relationship summary"
        );
        discovered_versions.retain(|_, result| result.is_ok());
        *self.discovery_cache.lock().await = discovered_versions;
        classifications
            .sort_by(|a, b| (&a.group, &a.kind, &a.version).cmp(&(&b.group, &b.kind, &b.version)));
        errors.sort_by(|a, b| {
            (&a.group, &a.version, &a.kind, &a.code).cmp(&(&b.group, &b.version, &b.kind, &b.code))
        });
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
        Ok(DiscoveryReport {
            context: self.context.clone(),
            connected: true,
            crd_count,
            confirmed_types,
            possible_types,
            excluded_types,
            managed_instance_count: resources
                .iter()
                .filter(|resource| {
                    resource.classification == Classification::ConfirmedManagedResource
                })
                .count(),
            resources,
            all_resources,
            graph,
            classifications,
            errors,
            relationship_errors: reference_errors,
            provider_images,
            provider_package_diagnostics,
        })
    }

    async fn discover_provider_packages(
        &self,
        resources: &[Resource],
    ) -> (HashMap<crate::core::ResourceId, String>, Vec<String>) {
        let api: Api<DynamicObject> = Api::all_with(
            self.client.clone(),
            &ApiResource::from_gvk_with_plural(
                &GroupVersionKind::gvk("pkg.crossplane.io", "v1", "Provider"),
                "providers",
            ),
        );
        let Ok(list) = api.list(&ListParams::default()).await else {
            return (
                HashMap::new(),
                vec!["Provider package discovery failed: could not list Providers from pkg.crossplane.io/v1.".into()],
            );
        };
        let packages: Vec<InstalledProvider> = list
            .items
            .into_iter()
            .filter_map(|item| {
                let value = serde_json::to_value(item).ok()?;
                let package = value.pointer("/spec/package")?.as_str()?.to_owned();
                let uid = value.pointer("/metadata/uid")?.as_str()?.to_owned();
                let name = value.pointer("/metadata/name")?.as_str()?.to_owned();
                Some(InstalledProvider { uid, name, package })
            })
            .collect();
        if packages.is_empty() {
            return (
                HashMap::new(),
                vec!["Provider package discovery returned no installed Providers.".into()],
            );
        }
        resolve_provider_packages(resources, &packages)
    }

    pub async fn list_events(&self, resource: &Resource) -> Result<Vec<Value>> {
        let api_resource =
            ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk("", "v1", "Event"), "events");
        let api: Api<DynamicObject> = match resource.namespace.as_deref() {
            Some(namespace) => Api::namespaced_with(self.client.clone(), namespace, &api_resource),
            None => Api::all_with(self.client.clone(), &api_resource),
        };
        let selector = if let Some(uid) = resource.uid.as_deref() {
            format!("involvedObject.uid={uid}")
        } else {
            format!(
                "involvedObject.name={},involvedObject.kind={}",
                resource.name, resource.kind
            )
        };
        let params = ListParams::default().fields(&selector);
        let list = api
            .list(&params)
            .await
            .context("Could not list Kubernetes Events; check Event list permissions")?;
        list.items
            .into_iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Could not decode Kubernetes Events")
    }

    async fn fetch_authoritative_targets(
        &self,
        resources: &[Resource],
        cache: &mut HashMap<String, DiscoveredVersion>,
    ) -> (Vec<Resource>, Vec<DiscoveryError>) {
        let mut fetched = Vec::new();
        let mut errors = Vec::new();
        let targets = relationships::authoritative_targets(resources);
        for target in targets {
            if resources.iter().any(|resource| {
                resource.api_version == target.api_version
                    && resource.kind == target.kind
                    && resource.name == target.name
                    && resource.namespace == target.namespace
            }) {
                continue;
            }
            let (group, version) = match target.api_version.split_once('/') {
                Some((group, version)) => (group.to_owned(), version.to_owned()),
                None => (String::new(), target.api_version.clone()),
            };
            let key = target.api_version.clone();
            if !cache.contains_key(&key) {
                let group_version = kube::core::GroupVersion::gv(&group, &version);
                let discovered = pinned_group(&self.client, &group_version)
                    .await
                    .map(|group| group.versioned_resources(&version))
                    .map_err(|error| kube_error_kind(&error));
                cache.insert(key.clone(), discovered);
            }
            let discovered = cache
                .get(&key)
                .expect("reference discovery cache entry exists");
            let version_resources = match discovered {
                Ok(resources) => resources,
                Err(code) => {
                    errors.push(reference_error(code, &target));
                    continue;
                }
            };
            let Some((api_resource, capabilities)) =
                version_resources.iter().find(|(resource, capabilities)| {
                    resource.group == group
                        && resource.version == version
                        && resource.kind == target.kind
                        && capabilities.supports_operation(verbs::GET)
                })
            else {
                errors.push(reference_error("resource_not_discovered", &target));
                continue;
            };
            let namespaced = matches!(capabilities.scope, Scope::Namespaced);
            let api: Api<DynamicObject> = if namespaced {
                let Some(namespace) = target.namespace.as_deref() else {
                    errors.push(reference_error("namespace_required", &target));
                    continue;
                };
                Api::namespaced_with(self.client.clone(), namespace, api_resource)
            } else {
                Api::all_with(self.client.clone(), api_resource)
            };
            match api.get(&target.name).await {
                Ok(object) => match serde_json::to_value(object) {
                    Ok(value) => {
                        let info = CrdInfo {
                            group: group.clone(),
                            kind: target.kind.clone(),
                            namespaced,
                            schema: Value::Null,
                            labels: Value::Null,
                            annotations: Value::Null,
                            owner_references: Value::Null,
                            categories: Vec::new(),
                            family: crossplane::family_for(&group, &target.kind),
                            api_version: target.api_version.clone(),
                            plural: api_resource.plural.clone(),
                            provider_config_default_kind: None,
                            vcd_vapp_name_reference: false,
                        };
                        fetched.push(crossplane::normalize(value, &info));
                    }
                    Err(_) => errors.push(reference_error("decode_error", &target)),
                },
                Err(error) => errors.push(reference_error(kube_error_kind(&error), &target)),
            }
        }
        (fetched, errors)
    }
}

fn reference_error(code: &str, target: &TargetReference) -> DiscoveryError {
    DiscoveryError {
        code: code.to_owned(),
        message: format!(
            "Could not resolve referenced resource {}/{}",
            target.kind, target.name
        ),
        group: target
            .api_version
            .split_once('/')
            .map(|(group, _)| group.to_owned()),
        version: Some(
            target
                .api_version
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned(),
        ),
        kind: Some(target.kind.clone()),
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

#[derive(Clone, Debug)]
struct InstalledProvider {
    uid: String,
    name: String,
    package: String,
}

fn resolve_provider_packages(
    resources: &[Resource],
    packages: &[InstalledProvider],
) -> (HashMap<crate::core::ResourceId, String>, Vec<String>) {
    let mut resolved = HashMap::new();
    let mut diagnostics = Vec::new();
    for config in resources
        .iter()
        .filter(|resource| resource.family == ResourceFamily::ProviderConfig)
    {
        let identity = format!("{}/{} [{}]", config.kind, config.name, config.group);
        let Some(owner_uid) = config.provider_owner_uid.as_deref() else {
            diagnostics.push(format!(
                "Provider package association unavailable for {identity}: owning ProviderRevision UID is missing."
            ));
            continue;
        };

        let revisions: Vec<_> = resources
            .iter()
            .filter(|resource| {
                resource.family == ResourceFamily::ProviderRevision
                    && resource.uid.as_deref() == Some(owner_uid)
            })
            .collect();
        let provider_uids: Vec<&str> = if revisions.is_empty() {
            packages
                .iter()
                .filter(|provider| provider.uid == owner_uid)
                .map(|provider| provider.uid.as_str())
                .collect()
        } else {
            if revisions.len() != 1 {
                diagnostics.push(format!(
                    "Provider package association ambiguous for {identity}: multiple ProviderRevisions share the owner UID."
                ));
                continue;
            }
            revisions[0]
                .original
                .pointer("/metadata/ownerReferences")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|owner| {
                    owner.get("kind").and_then(Value::as_str) == Some("Provider")
                        && owner
                            .get("apiVersion")
                            .and_then(Value::as_str)
                            .is_some_and(|version| version.starts_with("pkg.crossplane.io/"))
                })
                .filter_map(|owner| owner.get("uid").and_then(Value::as_str))
                .collect()
        };

        let matching_packages: Vec<_> = packages
            .iter()
            .filter(|provider| provider_uids.contains(&provider.uid.as_str()))
            .collect();
        match matching_packages.as_slice() {
            [provider] => {
                resolved.insert(config.id(), provider.package.clone());
                info!(provider = %provider.name, provider_config = %identity, "resolved ProviderConfig package by owner UID");
            }
            [] => diagnostics.push(format!(
                "Provider package association failed for {identity}: no installed Provider owns the referenced ProviderRevision."
            )),
            _ => diagnostics.push(format!(
                "Provider package association ambiguous for {identity}: multiple installed Providers own the referenced ProviderRevision."
            )),
        }
    }
    (resolved, diagnostics)
}

fn image_version(image: &str) -> Option<String> {
    let image = image.split('@').next()?;
    let last_component = image.rsplit('/').next()?;
    let (_, tag) = last_component.rsplit_once(':')?;
    (!tag.is_empty() && tag != "latest").then(|| tag.to_owned())
}

fn resolve_crossplane_version(deployments: &[Value]) -> CrossplaneVersionDiscovery {
    let controllers: Vec<_> = deployments
        .iter()
        .filter(|deployment| {
            deployment
                .pointer("/spec/template/spec/containers")
                .and_then(Value::as_array)
                .is_some_and(|containers| containers.iter().any(is_crossplane_controller_container))
        })
        .collect();
    let deployment = match controllers.as_slice() {
        [deployment] => deployment,
        [] => {
            return CrossplaneVersionDiscovery {
                version: None,
                diagnostic: Some(
                    "Crossplane version unavailable: no controller Deployment with a core start container was found.".into(),
                ),
            }
        }
        _ => {
            return CrossplaneVersionDiscovery {
                version: None,
                diagnostic: Some(format!(
                    "Crossplane version unavailable: found {} controller Deployments; refusing to choose one arbitrarily.",
                    controllers.len()
                )),
            }
        }
    };

    let label_version = deployment
        .pointer("/metadata/labels/app.kubernetes.io~1version")
        .and_then(Value::as_str)
        .and_then(normalize_version);
    let image_version = deployment
        .pointer("/spec/template/spec/containers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|container| is_crossplane_controller_container(container))
        .filter_map(|container| container.pointer("/image").and_then(Value::as_str))
        .filter_map(image_version)
        .filter_map(|version| normalize_version(&version))
        .next();
    let version = label_version.or(image_version);
    CrossplaneVersionDiscovery {
        diagnostic: version.is_none().then(|| {
            "Crossplane version unavailable: the controller Deployment has no reliable version label or image tag.".into()
        }),
        version,
    }
}

fn failed_crossplane_version_discovery(error_kind: &str) -> CrossplaneVersionDiscovery {
    CrossplaneVersionDiscovery {
        version: None,
        diagnostic: Some(format!(
            "Crossplane version unavailable: could not list Deployments ({error_kind})."
        )),
    }
}

fn is_crossplane_controller_container(container: &Value) -> bool {
    container
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|args| {
            args.first().and_then(Value::as_str) == Some("core")
                && args.get(1).and_then(Value::as_str) == Some("start")
        })
}

fn normalize_version(version: &str) -> Option<String> {
    let version = version.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let core = version.split(['-', '+']).next()?;
    (core.split('.').count() >= 2 && core.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .then(|| format!("v{version}"))
}

pub async fn kube_context(context: Option<&str>) -> Result<String> {
    match context {
        Some(context) => Config::from_kubeconfig(&kube::config::KubeConfigOptions {
            context: Some(context.to_owned()),
            ..Default::default()
        })
        .await
        .map_err(anyhow::Error::from),
        None => Config::infer().await.map_err(anyhow::Error::from),
    }
    .context(
        "Could not load Kubernetes configuration. Check KUBECONFIG and the current context.",
    )?;
    Ok(context.map(str::to_owned).unwrap_or_else(context_label))
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
    let provider_config_default_kind = schema
        .pointer("/properties/spec/properties/providerConfigRef/default/kind")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let vcd_vapp_name_reference = relationships::vcd_vapp_name_reference(&group, &kind, &schema);
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
        provider_config_default_kind,
        vcd_vapp_name_reference,
    };
    crd.family = crossplane::family_for_crd(&crd);
    Some(crd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider_config(group: &str, kind: &str, revision_uid: Option<&str>) -> Resource {
        let crd = CrdInfo {
            group: group.into(),
            kind: kind.into(),
            namespaced: kind == "ProviderConfig",
            schema: json!({}),
            labels: json!({}),
            annotations: json!({}),
            owner_references: revision_uid.map_or(json!([]), |uid| {
                json!([
                    {"apiVersion":"pkg.crossplane.io/v1", "kind":"ProviderRevision", "uid":uid}
                ])
            }),
            categories: vec![],
            family: ResourceFamily::ProviderConfig,
            api_version: format!("{group}/v1"),
            plural: "providerconfigs".into(),
            provider_config_default_kind: None,
            vcd_vapp_name_reference: false,
        };
        crossplane::normalize(
            json!({"apiVersion":format!("{group}/v1"),"kind":kind,"metadata":{"name":"default", "namespace":if kind == "ProviderConfig" {Some("default")} else {None}}}),
            &crd,
        )
    }

    fn provider_revision(uid: &str, provider_uids: &[&str]) -> Resource {
        let mut resource = provider_config("pkg.crossplane.io", "ProviderRevision", None);
        resource.family = ResourceFamily::ProviderRevision;
        resource.uid = Some(uid.into());
        resource.original["metadata"]["ownerReferences"] = json!(provider_uids
            .iter()
            .map(|provider_uid| json!({
                "apiVersion":"pkg.crossplane.io/v1", "kind":"Provider", "uid":provider_uid
            }))
            .collect::<Vec<_>>());
        resource
    }

    fn installed_provider(uid: &str, name: &str, package: &str) -> InstalledProvider {
        InstalledProvider {
            uid: uid.into(),
            name: name.into(),
            package: package.into(),
        }
    }

    #[test]
    fn provider_configs_resolve_packages_through_revision_owner_uids() {
        let talos = "ghcr.io/crossplane-contrib/provider-talos:v0.2.0";
        let vcd = "harbor.carekube.com/carekube/provider-vcd:dev-8634a2a2e85a";
        let configs = vec![
            provider_config("talos.crossplane.io", "ProviderConfig", Some("talos-rev")),
            provider_config("vcd.carekube.com", "ProviderConfig", Some("vcd-rev")),
            provider_config(
                "vcd.m.carekube.com",
                "ClusterProviderConfig",
                Some("vcd-rev"),
            ),
        ];
        let resources = vec![
            configs[0].clone(),
            configs[1].clone(),
            configs[2].clone(),
            provider_revision("talos-rev", &["talos-provider"]),
            provider_revision("vcd-rev", &["vcd-provider"]),
        ];
        let packages = vec![
            installed_provider("talos-provider", "provider-talos", talos),
            installed_provider("vcd-provider", "provider-vcd", vcd),
        ];
        let (resolved, diagnostics) = resolve_provider_packages(&resources, &packages);
        assert_eq!(
            resolved.get(&configs[0].id()).map(String::as_str),
            Some(talos)
        );
        assert_eq!(
            resolved.get(&configs[1].id()).map(String::as_str),
            Some(vcd)
        );
        assert_eq!(
            resolved.get(&configs[2].id()).map(String::as_str),
            Some(vcd)
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn provider_package_association_reports_missing_and_ambiguous_ownership() {
        let unresolved = provider_config("unknown.example.org", "ProviderConfig", None);
        let ambiguous = provider_config("ambiguous.example.org", "ProviderConfig", Some("rev"));
        let resources = vec![
            unresolved,
            ambiguous,
            provider_revision("rev", &["provider-a", "provider-b"]),
        ];
        let packages = vec![
            installed_provider("provider-a", "provider-a", "registry/a:v1"),
            installed_provider("provider-b", "provider-b", "registry/b:v1"),
        ];
        let (resolved, diagnostics) = resolve_provider_packages(&resources, &packages);
        assert!(resolved.is_empty());
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .any(|message| message.contains("UID is missing")));
        assert!(diagnostics
            .iter()
            .any(|message| message.contains("ambiguous")));
    }

    #[test]
    fn provider_package_association_reports_unmatched_owner_uid() {
        let config = provider_config("vcd.carekube.com", "ProviderConfig", Some("missing-rev"));
        let (resolved, diagnostics) = resolve_provider_packages(
            std::slice::from_ref(&config),
            &[installed_provider(
                "provider-vcd",
                "provider-vcd",
                "registry/vcd:v1",
            )],
        );
        assert!(resolved.is_empty());
        assert!(diagnostics[0].contains("association failed"));
    }

    #[test]
    fn parses_discovered_crd_scope_and_served_version() {
        let value = json!({"metadata":{"name":"buckets.s3.aws.m.upbound.io"},"spec":{"group":"s3.aws.m.upbound.io","scope":"Namespaced","names":{"kind":"Bucket","plural":"buckets"},"versions":[{"name":"v1beta1","served":true,"schema":{"openAPIV3Schema":{"properties":{"spec":{"properties":{"forProvider":{}}},"status":{"properties":{"atProvider":{}}}}}}}]}});
        let crd = parse_crd(&value).unwrap();
        assert!(crd.namespaced);
        assert_eq!(crd.api_version, "s3.aws.m.upbound.io/v1beta1");
        assert_eq!(crd.family, ResourceFamily::ManagedResource);
    }

    #[test]
    fn parses_vcd_v2_provider_config_default_and_schema_relationship() {
        let value = json!({
            "metadata": {"name":"vms.vapp.vcd.m.carekube.com", "ownerReferences":[
                {"apiVersion":"pkg.crossplane.io/v1","kind":"ProviderRevision","uid":"rev-1"}
            ]},
            "spec": {
                "group":"vapp.vcd.m.carekube.com", "scope":"Namespaced",
                "names":{"kind":"VM","plural":"vms","categories":["crossplane","managed","vcd"]},
                "versions":[{"name":"v1alpha1","served":true,"schema":{"openAPIV3Schema":{"properties":{
                    "spec":{"properties":{
                        "providerConfigRef":{"default":{"kind":"ClusterProviderConfig","name":"default"}},
                        "forProvider":{"properties":{"vappName":{"type":"string","description":"The vApp this VM belongs to."}}}
                    }},
                    "status":{"properties":{"atProvider":{}}}
                }}}}]
            }
        });
        let crd = parse_crd(&value).unwrap();
        assert_eq!(crd.family, ResourceFamily::ManagedResource);
        assert_eq!(
            crd.provider_config_default_kind.as_deref(),
            Some("ClusterProviderConfig")
        );
        assert!(crd.vcd_vapp_name_reference);
    }

    #[test]
    fn crossplane_definitions_and_internals_are_diagnostic_system_types() {
        for (group, kind) in [
            ("apiextensions.crossplane.io", "ManagedResourceDefinition"),
            ("pkg.crossplane.io", "ProviderRevision"),
            ("pkg.crossplane.io", "DeploymentRuntimeConfig"),
        ] {
            let value = json!({
                "metadata":{"name":format!("{}.example.org", kind.to_lowercase())},
                "spec":{"group":group,"scope":"Cluster","names":{"kind":kind,"plural":"objects","categories":["crossplane"]},
                    "versions":[{"name":"v1","served":true,"schema":{"openAPIV3Schema":{"type":"object"}}}]}
            });
            assert!(parse_crd(&value).unwrap().family.is_system());
        }
    }

    #[test]
    fn matches_discovered_api_by_group_version_kind_plural_and_list_verb() {
        use kube::{core::GroupVersionKind, discovery::ApiResource};

        let value: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/crossplane-vcd/vm-crd.json"
        ))
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

    #[test]
    fn crossplane_image_version_requires_a_real_tag() {
        assert_eq!(
            image_version("ghcr.io/crossplane/crossplane:v2.1.0"),
            Some("v2.1.0".into())
        );
        assert_eq!(
            image_version("ghcr.io/crossplane/crossplane@sha256:abc"),
            None
        );
        assert_eq!(image_version("ghcr.io/crossplane/crossplane:latest"), None);
    }

    #[test]
    fn deployment_version_is_normalized_and_rejects_unreliable_values() {
        assert_eq!(normalize_version("2.2.1"), Some("v2.2.1".into()));
        assert_eq!(normalize_version("v2.2.1"), Some("v2.2.1".into()));
        assert_eq!(normalize_version("latest"), None);
        assert_eq!(normalize_version(""), None);
    }

    fn deployment(
        name: &str,
        namespace: &str,
        args: &[&str],
        image: &str,
        version: Option<&str>,
    ) -> Value {
        let mut labels = json!({"app.kubernetes.io/name":"crossplane", "app.kubernetes.io/instance":"release-x"});
        if let Some(version) = version {
            labels["app.kubernetes.io/version"] = json!(version);
        }
        json!({
            "metadata":{"name":name,"namespace":namespace,"labels":labels},
            "spec":{"template":{"spec":{"containers":[{"name":"crossplane","args":args,"image":image}]}}}
        })
    }

    #[test]
    fn crossplane_deployment_label_wins_and_rbac_manager_is_excluded() {
        let controller = deployment(
            "custom-core-release",
            "custom-system-namespace",
            &["core", "start"],
            "xpkg.crossplane.io/crossplane/crossplane:v2.1.0",
            Some("2.2.1"),
        );
        let rbac = deployment(
            "custom-core-release-rbac-manager",
            "custom-system-namespace",
            &["rbac", "start"],
            "xpkg.crossplane.io/crossplane/crossplane:v2.2.1",
            Some("2.2.1"),
        );
        let result = resolve_crossplane_version(&[rbac, controller]);
        assert_eq!(result.version.as_deref(), Some("v2.2.1"));
        assert_eq!(result.diagnostic, None);
    }

    #[test]
    fn crossplane_controller_image_tag_is_used_when_label_is_missing() {
        let controller = deployment(
            "crossplane",
            "non-default-namespace",
            &["core", "start"],
            "xpkg.crossplane.io/crossplane/crossplane:v2.2.1",
            None,
        );
        assert_eq!(
            resolve_crossplane_version(&[controller]).version.as_deref(),
            Some("v2.2.1")
        );
    }

    #[test]
    fn provider_deployments_and_missing_crossplane_versions_are_diagnosed() {
        let provider = deployment(
            "provider-vcd",
            "crossplane-system",
            &["provider", "start"],
            "harbor.example/provider-vcd:v1.0.0",
            Some("1.0.0"),
        );
        let result = resolve_crossplane_version(&[provider]);
        assert_eq!(result.version, None);
        assert!(result
            .diagnostic
            .unwrap()
            .contains("no controller Deployment"));

        let unknown = deployment(
            "crossplane",
            "crossplane-system",
            &["core", "start"],
            "xpkg.crossplane.io/crossplane/crossplane:latest",
            None,
        );
        let result = resolve_crossplane_version(&[unknown]);
        assert_eq!(result.version, None);
        assert!(result.diagnostic.unwrap().contains("no reliable version"));
    }

    #[test]
    fn crossplane_version_discovery_reports_ambiguity_and_api_failure() {
        let first = deployment(
            "crossplane-a",
            "ns-a",
            &["core", "start"],
            "crossplane:v2.2.1",
            None,
        );
        let second = deployment(
            "crossplane-b",
            "ns-b",
            &["core", "start"],
            "crossplane:v2.2.1",
            None,
        );
        assert!(resolve_crossplane_version(&[first, second])
            .diagnostic
            .unwrap()
            .contains("refusing to choose"));
        assert!(failed_crossplane_version_discovery("forbidden")
            .diagnostic
            .unwrap()
            .contains("Deployments (forbidden)"));
    }
}
