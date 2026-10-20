use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::model::{Classification, Condition, ConditionStatus, Resource, ResourceFamily};

#[derive(Clone, Debug)]
pub struct CrdInfo {
    pub group: String,
    pub kind: String,
    pub namespaced: bool,
    pub schema: Value,
    pub labels: Value,
    pub annotations: Value,
    pub owner_references: Value,
    pub categories: Vec<String>,
    pub family: ResourceFamily,
    pub api_version: String,
    pub plural: String,
}

pub fn family_for(group: &str, kind: &str) -> ResourceFamily {
    match (group, kind) {
        ("apiextensions.crossplane.io", "CompositeResourceDefinition") => {
            ResourceFamily::CompositeResourceDefinition
        }
        ("apiextensions.crossplane.io", "Composition") => ResourceFamily::Composition,
        ("apiextensions.crossplane.io", "CompositionRevision") => {
            ResourceFamily::CompositionRevision
        }
        ("pkg.crossplane.io", "Provider") => ResourceFamily::Provider,
        ("pkg.crossplane.io", "ProviderRevision") => ResourceFamily::ProviderRevision,
        ("pkg.crossplane.io", "Function") => ResourceFamily::Function,
        ("pkg.crossplane.io", "FunctionRevision") => ResourceFamily::FunctionRevision,
        (_, "ProviderConfig" | "ClusterProviderConfig") => ResourceFamily::ProviderConfig,
        _ => ResourceFamily::Other,
    }
}

fn has_path(root: &Value, path: &[&str]) -> bool {
    path.iter()
        .try_fold(root, |value, key| value.get(*key))
        .is_some()
}

fn crossplane_crd_metadata(crd: &CrdInfo) -> bool {
    let labels = crd.labels.as_object();
    let annotations = crd.annotations.as_object();
    let owner = crd.owner_references.as_array().is_some_and(|refs| {
        refs.iter().any(|item| {
            item.get("apiVersion")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("pkg.crossplane.io/"))
        })
    });
    let crossplane_owner = crd.owner_references.as_array().is_some_and(|refs| {
        refs.iter().any(|item| {
            item.get("apiVersion")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("apiextensions.crossplane.io/"))
                && item.get("kind").and_then(Value::as_str) == Some("ManagedResourceDefinition")
        })
    });
    let managed_categories = crd
        .categories
        .iter()
        .any(|category| category == "crossplane")
        && crd.categories.iter().any(|category| category == "managed");
    owner
        || crossplane_owner
        || managed_categories
        || labels.is_some_and(|map| map.keys().any(|key| key.starts_with("pkg.crossplane.io/")))
        || annotations
            .is_some_and(|map| map.keys().any(|key| key.starts_with("meta.crossplane.io/")))
}

fn schema_has_for_provider(crd: &CrdInfo) -> bool {
    has_path(
        &crd.schema,
        &["properties", "spec", "properties", "forProvider"],
    )
}

fn schema_looks_managed(crd: &CrdInfo) -> bool {
    schema_has_for_provider(crd)
        && has_path(
            &crd.schema,
            &["properties", "status", "properties", "atProvider"],
        )
}

pub fn family_for_crd(crd: &CrdInfo) -> ResourceFamily {
    let core_family = family_for(&crd.group, &crd.kind);
    if core_family != ResourceFamily::Other {
        return core_family;
    }
    let xrd_owner = crd.owner_references.as_array().is_some_and(|refs| {
        refs.iter().any(|item| {
            item.get("apiVersion")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("apiextensions.crossplane.io/"))
                && item.get("kind").and_then(Value::as_str) == Some("CompositeResourceDefinition")
        })
    });
    let composite_schema = has_path(
        &crd.schema,
        &[
            "properties",
            "spec",
            "properties",
            "crossplane",
            "properties",
            "compositionRef",
        ],
    ) || has_path(
        &crd.schema,
        &["properties", "spec", "properties", "compositionRef"],
    );
    if xrd_owner || composite_schema {
        ResourceFamily::CompositeResource
    } else if schema_has_for_provider(crd) {
        ResourceFamily::ManagedResource
    } else {
        ResourceFamily::Other
    }
}

pub fn classify_crd(crd: &CrdInfo) -> Classification {
    if family_for_crd(crd) != ResourceFamily::ManagedResource {
        return Classification::NotManagedResource;
    }

    let managed_schema = schema_looks_managed(crd);
    let v2_convention = crd.group.split('.').any(|part| part == "m");
    if managed_schema && (v2_convention || crossplane_crd_metadata(crd)) {
        Classification::ConfirmedManagedResource
    } else if schema_has_for_provider(crd) {
        Classification::PossibleManagedResource
    } else {
        Classification::NotManagedResource
    }
}

pub fn classification_reasons(crd: &CrdInfo) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if schema_has_for_provider(crd) {
        reasons.push("schema contains spec.forProvider");
    }
    if has_path(
        &crd.schema,
        &["properties", "status", "properties", "atProvider"],
    ) {
        reasons.push("schema contains status.atProvider");
    }
    if has_path(
        &crd.schema,
        &["properties", "status", "properties", "conditions"],
    ) {
        reasons.push("schema contains status.conditions");
    }
    if crd
        .categories
        .iter()
        .any(|category| category == "crossplane")
    {
        reasons.push("CRD has crossplane category");
    }
    if crd.categories.iter().any(|category| category == "managed") {
        reasons.push("CRD has managed category");
    }
    if crd.owner_references.as_array().is_some_and(|refs| {
        refs.iter().any(|item| {
            item.get("apiVersion")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("pkg.crossplane.io/"))
        })
    }) {
        reasons.push("CRD is owned by a Crossplane ProviderRevision");
    }
    if crd.owner_references.as_array().is_some_and(|refs| {
        refs.iter().any(|item| {
            item.get("apiVersion")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("apiextensions.crossplane.io/"))
                && item.get("kind").and_then(Value::as_str) == Some("ManagedResourceDefinition")
        })
    }) {
        reasons.push("CRD is owned by a Crossplane ManagedResourceDefinition");
    }
    if crd
        .labels
        .as_object()
        .is_some_and(|map| map.keys().any(|key| key.starts_with("pkg.crossplane.io/")))
    {
        reasons.push("CRD has Crossplane package metadata label");
    }
    if crd
        .annotations
        .as_object()
        .is_some_and(|map| map.keys().any(|key| key.starts_with("meta.crossplane.io/")))
    {
        reasons.push("CRD has Crossplane metadata annotation");
    }
    if crd.group.split('.').any(|part| part == "m") {
        reasons.push("API group uses Crossplane v2 managed-resource convention");
    }
    reasons
}

pub fn object_classification_reasons(crd: &CrdInfo, object: &Value) -> Vec<&'static str> {
    let mut reasons = classification_reasons(crd);
    let metadata = object.get("metadata").unwrap_or(&Value::Null);
    let has_external_name = metadata
        .pointer("/annotations/crossplane.io~1external-name")
        .and_then(Value::as_str)
        .is_some();
    let has_provider_config_ref = has_path(object, &["spec", "providerConfigRef"]);
    let has_condition = object
        .pointer("/status/conditions")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("Ready" | "Synced")
                )
            })
        });
    let has_owner = metadata
        .get("ownerReferences")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    for (present, positive, absent) in [
        (
            has_external_name,
            "object has crossplane.io/external-name",
            "object lacks crossplane.io/external-name",
        ),
        (
            has_provider_config_ref,
            "object has spec.providerConfigRef",
            "object lacks spec.providerConfigRef",
        ),
        (
            has_condition,
            "object has Ready or Synced condition",
            "object lacks Ready and Synced conditions",
        ),
        (
            has_owner,
            "object has owner references",
            "object lacks owner references",
        ),
    ] {
        reasons.push(if present { positive } else { absent });
    }
    if [
        has_external_name,
        has_provider_config_ref,
        has_condition,
        has_owner,
    ]
    .into_iter()
    .filter(|signal| *signal)
    .count()
        < 2
    {
        reasons.push("fewer than two object-level signals are present");
    }
    reasons
}

fn parse_condition(value: Option<&Value>, expected_type: &str) -> Condition {
    let Some(condition) = value.and_then(Value::as_array).and_then(|items| {
        items
            .iter()
            .find(|item| item.get("type").and_then(Value::as_str) == Some(expected_type))
    }) else {
        return Condition {
            status: ConditionStatus::Missing,
            reason: None,
            message: None,
            last_transition_time: None,
        };
    };
    let status = match condition.get("status").and_then(Value::as_str) {
        Some("True") => ConditionStatus::True,
        Some("False") => ConditionStatus::False,
        Some("Unknown") => ConditionStatus::Unknown,
        _ => ConditionStatus::Unknown,
    };
    Condition {
        status,
        reason: condition
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
        message: condition
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        last_transition_time: condition
            .get("lastTransitionTime")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

pub fn normalize(value: Value, crd: &CrdInfo) -> Resource {
    let metadata = value.get("metadata").unwrap_or(&Value::Null);
    let conditions = value.pointer("/status/conditions");
    let get_condition = |name: &str| parse_condition(conditions, name);
    let created_at = metadata
        .get("creationTimestamp")
        .and_then(Value::as_str)
        .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.with_timezone(&Utc));
    let external_name = metadata
        .pointer("/annotations/crossplane.io~1external-name")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let synced = get_condition("Synced");
    let ready = get_condition("Ready");
    let classification = classify_object(crd, &value);
    Resource {
        api_version: value
            .get("apiVersion")
            .and_then(Value::as_str)
            .unwrap_or(&crd.api_version)
            .to_owned(),
        kind: value
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or(&crd.kind)
            .to_owned(),
        name: metadata
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>")
            .to_owned(),
        namespace: metadata
            .get("namespace")
            .and_then(Value::as_str)
            .map(str::to_owned),
        uid: metadata
            .get("uid")
            .and_then(Value::as_str)
            .map(str::to_owned),
        created_at,
        external_name,
        synced,
        ready,
        family: crd.family.clone(),
        classification,
        original: value,
    }
}

pub fn normalize_objects(values: impl IntoIterator<Item = Value>, crd: &CrdInfo) -> Vec<Resource> {
    values
        .into_iter()
        .map(|value| normalize(value, crd))
        .collect()
}

fn classify_object(crd: &CrdInfo, object: &Value) -> Classification {
    let base = classify_crd(crd);
    if base == Classification::ConfirmedManagedResource || !schema_has_for_provider(crd) {
        return base;
    }
    let metadata = object.get("metadata").unwrap_or(&Value::Null);
    let annotations = metadata.get("annotations").and_then(Value::as_object);
    let external = annotations.is_some_and(|map| map.contains_key("crossplane.io/external-name"));
    let provider_ref = has_path(object, &["spec", "providerConfigRef"]);
    let conditions = object
        .pointer("/status/conditions")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("Ready" | "Synced")
                )
            })
        });
    let owned = metadata
        .get("ownerReferences")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    if [external, provider_ref, conditions, owned]
        .into_iter()
        .filter(|signal| *signal)
        .count()
        >= 2
    {
        Classification::ConfirmedManagedResource
    } else {
        Classification::PossibleManagedResource
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn crd(group: &str, namespaced: bool, metadata: Value) -> CrdInfo {
        CrdInfo {
            group: group.into(),
            kind: "Bucket".into(),
            namespaced,
            schema: json!({"properties":{"spec":{"properties":{"forProvider":{}}},"status":{"properties":{"atProvider":{}}}}}),
            labels: metadata.get("labels").cloned().unwrap_or(Value::Null),
            annotations: metadata.get("annotations").cloned().unwrap_or(Value::Null),
            owner_references: metadata
                .get("ownerReferences")
                .cloned()
                .unwrap_or(Value::Null),
            categories: vec![],
            family: ResourceFamily::ManagedResource,
            api_version: format!("{group}/v1beta1"),
            plural: "buckets".into(),
        }
    }

    #[test]
    fn classifies_upjet_v1_from_crd_controller_ownership() {
        let info = crd(
            "s3.aws.upbound.io",
            false,
            json!({"ownerReferences":[{"apiVersion":"pkg.crossplane.io/v1","kind":"ProviderRevision"}]}),
        );
        assert_eq!(
            classify_crd(&info),
            Classification::ConfirmedManagedResource
        );
    }

    #[test]
    fn classifies_v2_namespaced_mr_from_generic_group_convention() {
        let info = crd("s3.aws.m.upbound.io", true, json!({}));
        assert_eq!(
            classify_crd(&info),
            Classification::ConfirmedManagedResource
        );
    }

    #[test]
    fn schema_only_is_possible_and_unrelated_crd_is_not_managed() {
        let info = crd("storage.example.org", false, json!({}));
        assert_eq!(classify_crd(&info), Classification::PossibleManagedResource);
        let mut unrelated = info;
        unrelated.schema = json!({"properties":{"spec":{"properties":{"parameters":{}}}}});
        assert_eq!(classify_crd(&unrelated), Classification::NotManagedResource);
    }

    #[test]
    fn conditions_and_missing_or_malformed_data_are_defensive() {
        let info = crd("s3.aws.m.upbound.io", true, json!({}));
        let value = json!({"apiVersion":"s3.aws.m.upbound.io/v1","kind":"Bucket","metadata":{"name":"demo"},"status":{"conditions":[{"type":"Ready","status":"False","reason":"Creating","message":"waiting"}]}});
        let resource = normalize(value, &info);
        assert_eq!(resource.ready.status, ConditionStatus::False);
        assert_eq!(resource.ready.reason.as_deref(), Some("Creating"));
        assert_eq!(resource.synced.status, ConditionStatus::Missing);
        let malformed = json!({"status":{"conditions":{"Ready":true}}});
        assert_eq!(
            parse_condition(malformed.pointer("/status/conditions"), "Ready").status,
            ConditionStatus::Missing
        );
    }

    #[test]
    fn identifies_crossplane_core_families() {
        assert_eq!(
            family_for("apiextensions.crossplane.io", "Composition"),
            ResourceFamily::Composition
        );
        assert_eq!(
            family_for("pkg.crossplane.io", "FunctionRevision"),
            ResourceFamily::FunctionRevision
        );
    }

    #[test]
    fn recognizes_custom_composite_resources_from_xrd_schema_conventions() {
        let mut info = crd("platform.example.org", true, json!({}));
        info.schema = json!({"properties":{"spec":{"properties":{"crossplane":{"properties":{"compositionRef":{"type":"object"}}}}}}});
        assert_eq!(family_for_crd(&info), ResourceFamily::CompositeResource);
        assert_eq!(classify_crd(&info), Classification::NotManagedResource);
    }

    #[test]
    fn object_evidence_promotes_a_possible_managed_resource() {
        let info = crd("storage.example.org", false, json!({}));
        let object = json!({"metadata":{"name":"candidate","annotations":{"crossplane.io/external-name":"external"}},"spec":{"forProvider":{},"providerConfigRef":{"name":"default"}}});
        let resource = normalize(object, &info);
        assert_eq!(
            resource.classification,
            Classification::ConfirmedManagedResource
        );
    }

    #[test]
    fn possible_resource_explains_missing_classification_evidence() {
        let info = crd("storage.example.org", false, json!({}));
        let object = json!({"metadata":{"name":"candidate"},"spec":{"forProvider":{}}});
        let resource = normalize(object.clone(), &info);
        assert_eq!(
            resource.classification,
            Classification::PossibleManagedResource
        );
        let reasons = object_classification_reasons(&info, &object);
        assert!(reasons.contains(&"fewer than two object-level signals are present"));
        assert!(reasons.contains(&"object lacks Ready and Synced conditions"));
    }

    #[test]
    fn upjet_v1_cluster_scoped_fixture_normalizes_and_confirms() {
        let crd: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-v1/upjet-crd.json"
        ))
        .unwrap();
        let crd = crate::kubernetes::parse_crd(&crd).unwrap();
        assert!(!crd.namespaced);
        assert_eq!(classify_crd(&crd), Classification::ConfirmedManagedResource);

        let object: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-v1/upjet-mr.json"
        ))
        .unwrap();
        let resource = normalize(object, &crd);
        assert_eq!(
            resource.classification,
            Classification::ConfirmedManagedResource
        );
        assert_eq!(resource.namespace, None);
        assert_eq!(resource.external_name.as_deref(), Some("external-bucket"));
        assert_eq!(resource.synced.status, ConditionStatus::True);
        assert_eq!(resource.ready.status, ConditionStatus::False);
        assert!(resource.original.get("spec").is_some());
    }

    #[test]
    fn custom_vcd_upjet_fixtures_reach_confirmed_normalized_resource_list() {
        let cases = [
            (
                include_str!("../tests/fixtures/crossplane-vcd/vm-crd.json"),
                include_str!("../tests/fixtures/crossplane-vcd/vm.json"),
                "vapp.vcd.carekube.com/v1alpha1",
                "VM",
                "seed-cp01",
            ),
            (
                include_str!("../tests/fixtures/crossplane-vcd/vapp-crd.json"),
                include_str!("../tests/fixtures/crossplane-vcd/vapp.json"),
                "vcd.vcd.carekube.com/v1alpha1",
                "Vapp",
                "seed",
            ),
            (
                include_str!("../tests/fixtures/crossplane-vcd/natrule-crd.json"),
                include_str!("../tests/fixtures/crossplane-vcd/natrule.json"),
                "nsxt.vcd.carekube.com/v1alpha1",
                "NATRule",
                "talos-bootstrap-cp01",
            ),
        ];
        let mut normalized = Vec::new();
        for (crd_json, object_json, api_version, kind, name) in cases {
            let crd_value: Value = serde_json::from_str(crd_json).unwrap();
            let crd = crate::kubernetes::parse_crd(&crd_value).unwrap();
            assert_eq!(classify_crd(&crd), Classification::ConfirmedManagedResource);
            assert_eq!(crd.api_version, api_version);
            assert!(!crd.namespaced);
            let object: Value = serde_json::from_str(object_json).unwrap();
            normalized.extend(normalize_objects([object], &crd));
            assert_eq!(crd.kind, kind);
            assert_eq!(normalized.last().unwrap().name, name);
        }
        let vm_crd_value: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm-crd.json"))
                .unwrap();
        let vm_crd = crate::kubernetes::parse_crd(&vm_crd_value).unwrap();
        let mut vm_two: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm.json")).unwrap();
        vm_two["metadata"]["name"] = serde_json::json!("seed-cp02");
        normalized.extend(normalize_objects([vm_two], &vm_crd));

        let nat_crd_value: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-vcd/natrule-crd.json"
        ))
        .unwrap();
        let nat_crd = crate::kubernetes::parse_crd(&nat_crd_value).unwrap();
        let mut nat_two: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-vcd/natrule.json"
        ))
        .unwrap();
        nat_two["metadata"]["name"] = serde_json::json!("talos-bootstrap-cp02");
        normalized.extend(normalize_objects([nat_two], &nat_crd));

        assert_eq!(normalized.len(), 5);
        let names: Vec<_> = normalized
            .iter()
            .map(|resource| resource.name.as_str())
            .collect();
        assert!(names.contains(&"seed-cp01"));
        assert!(names.contains(&"seed-cp02"));
        assert!(names.contains(&"seed"));
        assert!(names.contains(&"talos-bootstrap-cp01"));
        assert!(names.contains(&"talos-bootstrap-cp02"));
        assert!(normalized.iter().all(|resource| {
            resource.classification == Classification::ConfirmedManagedResource
                && resource.namespace.is_none()
        }));
        for nat_rule in normalized
            .iter()
            .filter(|resource| resource.kind == "NATRule")
        {
            assert_eq!(nat_rule.synced.status, ConditionStatus::False);
            assert_eq!(nat_rule.ready.status, ConditionStatus::True);
        }
    }

    #[test]
    fn equal_kinds_in_distinct_api_groups_remain_distinct_normalized_resources() {
        let crd_vm: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm-crd.json"))
                .unwrap();
        let first_crd = crate::kubernetes::parse_crd(&crd_vm).unwrap();
        let mut second_crd = first_crd.clone();
        second_crd.group = "other.vcd.carekube.com".into();
        second_crd.api_version = "other.vcd.carekube.com/v1alpha1".into();
        let first: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm.json")).unwrap();
        let mut second = first.clone();
        second["apiVersion"] = serde_json::json!("other.vcd.carekube.com/v1alpha1");
        let mut normalized = normalize_objects([first], &first_crd);
        normalized.extend(normalize_objects([second], &second_crd));
        assert_eq!(normalized.len(), 2);
        assert_ne!(normalized[0].api_version, normalized[1].api_version);
        assert_eq!(normalized[0].kind, normalized[1].kind);
        assert_eq!(normalized[0].name, normalized[1].name);
    }

    #[test]
    fn vcd_category_and_managed_definition_owner_are_valid_crossplane_evidence() {
        let crd: Value =
            serde_json::from_str(include_str!("../tests/fixtures/crossplane-vcd/vm-crd.json"))
                .unwrap();
        let mut info = crate::kubernetes::parse_crd(&crd).unwrap();
        info.owner_references = serde_json::json!([{
            "apiVersion": "apiextensions.crossplane.io/v1alpha1",
            "kind": "ManagedResourceDefinition"
        }]);
        assert_eq!(
            classify_crd(&info),
            Classification::ConfirmedManagedResource
        );
        info.owner_references = Value::Null;
        assert_eq!(
            classify_crd(&info),
            Classification::ConfirmedManagedResource
        );
    }

    #[test]
    fn crossplane_v2_namespaced_fixture_is_discovered_and_normalized() {
        let crd: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-v2/namespaced-crd.json"
        ))
        .unwrap();
        let crd = crate::kubernetes::parse_crd(&crd).unwrap();
        assert!(crd.namespaced);
        assert_eq!(classify_crd(&crd), Classification::ConfirmedManagedResource);

        let object: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/crossplane-v2/namespaced-mr.json"
        ))
        .unwrap();
        let resource = normalize(object, &crd);
        assert_eq!(resource.namespace.as_deref(), Some("team-a"));
        assert_eq!(resource.synced.status, ConditionStatus::Unknown);
        assert_eq!(resource.ready.status, ConditionStatus::True);
    }

    #[test]
    fn non_crossplane_crd_fixture_is_not_misclassified() {
        let value: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/non-crossplane/non-crossplane-crd.json"
        ))
        .unwrap();
        let crd = crate::kubernetes::parse_crd(&value).unwrap();
        assert_eq!(crd.family, ResourceFamily::Other);
        assert_eq!(classify_crd(&crd), Classification::NotManagedResource);
    }
}
