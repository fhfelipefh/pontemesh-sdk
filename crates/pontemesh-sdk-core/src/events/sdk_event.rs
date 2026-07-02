#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkEvent {
    AccessPackageCreated { package_id: String },
    FragmentValidated { fragment_index: usize },
    SourceFailed { source_id: String },
    ObjectSynced { bucket: String, key: String },
}
