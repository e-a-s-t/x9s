use crate::core::{Classification, DiscoveryReport, Resource};

use super::{args::OutputFormat, output};

pub fn write(
    report: &DiscoveryReport,
    namespace: Option<&str>,
    _all_namespaces: bool,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let resources: Vec<_> = report
        .resources
        .iter()
        .filter(|resource| {
            resource.classification == Classification::ConfirmedManagedResource
                && matches_namespace(resource, namespace)
        })
        .map(Resource::managed_resource)
        .collect();

    match format {
        OutputFormat::Json => output::json(&resources)?,
        OutputFormat::Table => write_table(report, namespace, &resources)?,
    }
    if report.is_partial() {
        super::write_errors(&report.errors, format == OutputFormat::Json);
    }
    Ok(())
}

fn write_table(
    report: &DiscoveryReport,
    namespace: Option<&str>,
    resources: &[crate::core::ManagedResource],
) -> anyhow::Result<()> {
    use std::io::Write;

    let table = format_table(resources);
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(table.as_bytes())?;
    if resources.is_empty() && !report.is_partial() {
        let filter = namespace
            .map(|ns| format!(" in namespace {ns}"))
            .unwrap_or_default();
        eprintln!("No confirmed Managed Resources found{filter}.");
    }
    Ok(())
}

fn matches_namespace(resource: &Resource, namespace: Option<&str>) -> bool {
    namespace.is_none_or(|namespace| {
        resource
            .namespace
            .as_deref()
            .is_none_or(|value| value == namespace)
    })
}

fn format_table(resources: &[crate::core::ManagedResource]) -> String {
    use std::fmt::Write as _;

    let widths = [
        resources
            .iter()
            .map(|item| item.kind.len())
            .max()
            .unwrap_or(0)
            .max("KIND".len()),
        resources
            .iter()
            .map(|item| item.name.len())
            .max()
            .unwrap_or(0)
            .max("NAME".len()),
        resources
            .iter()
            .map(|item| item.namespace.as_deref().unwrap_or("-").len())
            .max()
            .unwrap_or(0)
            .max("NAMESPACE".len()),
    ];
    let mut table = String::new();
    let _ = writeln!(
        table,
        "{:<kind_width$}  {:<name_width$}  {:<namespace_width$}  SYNCED  READY",
        "KIND",
        "NAME",
        "NAMESPACE",
        kind_width = widths[0],
        name_width = widths[1],
        namespace_width = widths[2],
    );
    for resource in resources {
        let _ = writeln!(
            table,
            "{:<kind_width$}  {:<name_width$}  {:<namespace_width$}  {:<6}  {}",
            resource.kind,
            resource.name,
            resource.namespace.as_deref().unwrap_or("-"),
            resource.synced.status.label(),
            resource.ready.status.label(),
            kind_width = widths[0],
            name_width = widths[1],
            namespace_width = widths[2],
        );
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Condition, ConditionStatus, ResourceFamily};
    use serde_json::Value;

    fn resource(namespace: Option<&str>) -> Resource {
        Resource {
            api_version: "example.org/v1".into(),
            group: "example.org".into(),
            version: "v1".into(),
            kind: "Widget".into(),
            resource: "widgets".into(),
            name: "sample".into(),
            namespace: namespace.map(str::to_owned),
            uid: None,
            created_at: None,
            external_name: None,
            synced: Condition {
                status: ConditionStatus::Missing,
                reason: None,
                message: None,
                last_transition_time: None,
            },
            ready: Condition {
                status: ConditionStatus::True,
                reason: None,
                message: None,
                last_transition_time: None,
            },
            family: ResourceFamily::ManagedResource,
            classification: Classification::ConfirmedManagedResource,
            original: Value::Null,
        }
    }

    #[test]
    fn namespace_filter_keeps_cluster_scoped_resources() {
        assert!(matches_namespace(
            &resource(None),
            Some("crossplane-system")
        ));
        assert!(matches_namespace(
            &resource(Some("crossplane-system")),
            Some("crossplane-system")
        ));
        assert!(!matches_namespace(
            &resource(Some("default")),
            Some("crossplane-system")
        ));
    }

    #[test]
    fn table_output_has_aligned_columns_and_no_control_sequences() {
        let table = format_table(&[resource(None).managed_resource()]);
        assert!(table.contains("KIND    NAME    NAMESPACE  SYNCED  READY"));
        assert!(table.contains("Widget  sample  -          Missing  True"));
        assert!(!table.contains('\u{1b}'));
    }
}
