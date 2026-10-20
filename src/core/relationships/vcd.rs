use serde_json::Value;

use super::{Relationship, RelationshipType};
use crate::core::models::Resource;

/// Recognize the provider's documented scalar relationship from the installed
/// OpenAPI schema. This stays in the VCD resolver instead of generic parsing.
pub(super) fn schema_declares_vapp_name_relationship(
    group: &str,
    kind: &str,
    schema: &Value,
) -> bool {
    let description = schema
        .pointer("/properties/spec/properties/forProvider/properties/vappName/description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    group.contains(".vcd.")
        && group.ends_with(".carekube.com")
        && kind == "VM"
        && description.contains("vapp")
        && description.contains("belongs to")
}

/// Resolve VCD's declared VM `vappName` relation against VApp external names.
/// Provider ownership, ProviderConfig identity, and namespace must all agree.
pub(super) fn resolve(resources: &[Resource], existing: &[Relationship]) -> Vec<Relationship> {
    resources
        .iter()
        .filter(|resource| resource.vcd_vapp_name_reference)
        .filter_map(|vm| {
            let vapp_name = vm
                .original
                .pointer("/spec/forProvider/vappName")
                .and_then(Value::as_str)?;
            let vm_id = vm.id();
            let provider_config = existing.iter().find_map(|edge| {
                (edge.from == vm_id && edge.relation == RelationshipType::ManagedBy)
                    .then(|| edge.to.clone())
                    .flatten()
            });
            let candidates: Vec<_> = resources
                .iter()
                .filter(|candidate| {
                    candidate.kind == "Vapp"
                        && candidate.external_name.as_deref() == Some(vapp_name)
                        && candidate.provider_owner_uid == vm.provider_owner_uid
                        && vm.provider_owner_uid.is_some()
                        && candidate.namespace == vm.namespace
                        && provider_config.as_ref().is_some_and(|provider| {
                            existing.iter().any(|edge| {
                                edge.from == candidate.id()
                                    && edge.relation == RelationshipType::ManagedBy
                                    && edge.to.as_ref() == Some(provider)
                            })
                        })
                })
                .map(Resource::id)
                .collect();
            Some(Relationship {
                from: vm_id,
                to: (candidates.len() == 1).then(|| candidates[0].clone()),
                relation: RelationshipType::BelongsTo,
                unresolved: (candidates.len() != 1)
                    .then(|| format!("VApp external name {vapp_name} did not resolve uniquely")),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::schema_declares_vapp_name_relationship;
    use serde_json::json;

    #[test]
    fn requires_installed_schema_semantics_and_matching_provider_api() {
        let schema = json!({"properties":{"spec":{"properties":{"forProvider":{"properties":{"vappName":{"description":"The vApp this VM belongs to."}}}}}}});
        assert!(schema_declares_vapp_name_relationship(
            "vapp.vcd.m.carekube.com",
            "VM",
            &schema
        ));
        assert!(!schema_declares_vapp_name_relationship(
            "compute.example.org",
            "VM",
            &schema
        ));
        assert!(!schema_declares_vapp_name_relationship(
            "vapp.vcd.m.carekube.com",
            "Other",
            &schema
        ));
    }
}
