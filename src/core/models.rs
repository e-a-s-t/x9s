use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
    Missing,
}

impl ConditionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::True => "True",
            Self::False => "False",
            Self::Unknown => "Unknown",
            Self::Missing => "Missing",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Condition {
    pub status: ConditionStatus,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub last_transition_time: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[allow(clippy::enum_variant_names)] // The variant names mirror the classification contract.
pub enum Classification {
    ConfirmedManagedResource,
    PossibleManagedResource,
    NotManagedResource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceFamily {
    ManagedResource,
    CompositeResource,
    CompositeResourceDefinition,
    Composition,
    CompositionRevision,
    Provider,
    ProviderRevision,
    Function,
    FunctionRevision,
    ProviderConfig,
    System,
    Other,
}

impl ResourceFamily {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ManagedResource => "Managed Resource",
            Self::CompositeResource => "Composite Resource",
            Self::CompositeResourceDefinition => "CompositeResourceDefinition",
            Self::Composition => "Composition",
            Self::CompositionRevision => "CompositionRevision",
            Self::Provider => "Provider",
            Self::ProviderRevision => "ProviderRevision",
            Self::Function => "Function",
            Self::FunctionRevision => "FunctionRevision",
            Self::ProviderConfig => "ProviderConfig",
            Self::System => "Crossplane System",
            Self::Other => "Other",
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(
            self,
            Self::CompositeResourceDefinition
                | Self::Composition
                | Self::CompositionRevision
                | Self::Provider
                | Self::ProviderRevision
                | Self::Function
                | Self::FunctionRevision
                | Self::System
        )
    }
}

#[derive(Clone, Debug)]
pub struct Resource {
    pub api_version: String,
    pub group: String,
    pub version: String,
    pub kind: String,
    pub resource: String,
    pub name: String,
    pub namespace: Option<String>,
    pub uid: Option<String>,
    /// UID of the installed Crossplane Provider/ProviderRevision that owns the
    /// CRD, used to scope name-only providerConfigRef resolution safely.
    pub provider_owner_uid: Option<String>,
    /// Kind default declared by the Managed Resource CRD's providerConfigRef schema.
    pub provider_config_default_kind: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub external_name: Option<String>,
    pub synced: Condition,
    pub ready: Condition,
    pub family: ResourceFamily,
    /// True when the installed CRD schema declares VCD VM-to-vApp semantics.
    pub vcd_vapp_name_reference: bool,
    pub classification: Classification,
    #[allow(dead_code)] // Retained for future object inspection views.
    pub original: Value,
}

/// Identity for a Kubernetes object in the relationship graph. UID is retained
/// separately from the address so a recreated object can be distinguished.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId {
    pub api_version: String,
    pub group: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
    pub uid: Option<String>,
}

impl Resource {
    pub fn id(&self) -> ResourceId {
        ResourceId {
            api_version: self.api_version.clone(),
            group: self.group.clone(),
            kind: self.kind.clone(),
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            uid: self.uid.clone(),
        }
    }
}

/// Presentation-safe resource shape shared by the TUI and command-line output.
/// The original Kubernetes object is intentionally excluded to prevent leaking
/// arbitrary object fields, including Secret-like values, into JSON output.
#[derive(Clone, Debug, Serialize)]
pub struct ManagedResource {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub resource: String,
    pub namespace: Option<String>,
    pub name: String,
    pub external_name: Option<String>,
    pub synced: ManagedCondition,
    pub ready: ManagedCondition,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagedCondition {
    pub status: ConditionStatus,
}

impl Resource {
    pub fn managed_resource(&self) -> ManagedResource {
        ManagedResource {
            group: self.group.clone(),
            version: self.version.clone(),
            kind: self.kind.clone(),
            resource: self.resource.clone(),
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            external_name: self.external_name.clone(),
            synced: ManagedCondition {
                status: self.synced.status.clone(),
            },
            ready: ManagedCondition {
                status: self.ready.status.clone(),
            },
        }
    }
}

pub fn sort_resources(resources: &mut [Resource]) {
    resources.sort_by(|a, b| {
        (&a.kind, &a.group, &a.version, &a.namespace, &a.name).cmp(&(
            &b.kind,
            &b.group,
            &b.version,
            &b.namespace,
            &b.name,
        ))
    });
}
