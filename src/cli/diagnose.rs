use serde::Serialize;

use crate::core::{DiscoveryReport, TypeClassification};

use super::{args::OutputFormat, output};

#[derive(Serialize)]
struct DiagnosticOutput<'a> {
    context: &'a str,
    connected: bool,
    discovered_crds: usize,
    confirmed_managed_resource_types: usize,
    possible_managed_resource_types: usize,
    excluded_resource_types: usize,
    managed_resource_instances: usize,
    classifications: &'a [TypeClassification],
    errors: &'a [crate::core::DiscoveryError],
}

pub fn write(report: &DiscoveryReport, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            let output = DiagnosticOutput {
                context: &report.context,
                connected: report.connected,
                discovered_crds: report.crd_count,
                confirmed_managed_resource_types: report.confirmed_types,
                possible_managed_resource_types: report.possible_types,
                excluded_resource_types: report.excluded_types,
                managed_resource_instances: report.managed_instance_count,
                classifications: &report.classifications,
                errors: &report.errors,
            };
            output::json(&output)?;
        }
        OutputFormat::Table => {
            println!("Context: {}", report.context);
            println!("Kubernetes connected: {}", report.connected);
            println!("Discovered CRDs: {}", report.crd_count);
            println!(
                "Confirmed Managed Resource types: {}",
                report.confirmed_types
            );
            println!("Possible Managed Resource types: {}", report.possible_types);
            println!("Excluded resource types: {}", report.excluded_types);
            println!(
                "Managed Resource instances: {}",
                report.managed_instance_count
            );
            if report.classifications.is_empty() {
                println!("Classifications: none");
            } else {
                println!("Classifications:");
                for item in &report.classifications {
                    println!(
                        "  {}/{} {}: {:?} ({})",
                        item.group,
                        item.version,
                        item.kind,
                        item.classification,
                        item.reasons.join(", ")
                    );
                }
            }
            if !report.errors.is_empty() {
                println!("Discovery and listing errors:");
                for error in &report.errors {
                    println!("  {}: {}", error.code, error.message);
                }
            }
        }
    }
    if report.is_partial() {
        super::write_errors(&report.errors, format == OutputFormat::Json);
    }
    Ok(())
}
