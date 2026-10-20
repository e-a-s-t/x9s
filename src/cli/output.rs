use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

pub fn json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    serde_json::to_writer_pretty(std::io::stdout().lock(), value)?;
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::core::{ConditionStatus, ManagedCondition, ManagedResource};

    #[test]
    fn managed_resource_json_is_stable_and_excludes_arbitrary_condition_messages() {
        let resource = ManagedResource {
            group: "example.org".into(),
            version: "v1".into(),
            kind: "Widget".into(),
            resource: "widgets".into(),
            namespace: None,
            name: "sample".into(),
            external_name: None,
            synced: ManagedCondition {
                status: ConditionStatus::Missing,
            },
            ready: ManagedCondition {
                status: ConditionStatus::Unknown,
            },
        };
        let json = serde_json::to_string(&resource).unwrap();
        assert_eq!(
            json,
            r#"{"group":"example.org","version":"v1","kind":"Widget","resource":"widgets","namespace":null,"name":"sample","external_name":null,"synced":{"status":"Missing"},"ready":{"status":"Unknown"}}"#
        );
        assert!(!json.contains('\u{1b}'));
        assert!(!json.contains("message"));
    }
}
