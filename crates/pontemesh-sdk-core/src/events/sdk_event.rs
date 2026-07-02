#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkEvent {
    AccessPackageCreated {
        package_id: String,
    },
    FragmentValidated {
        fragment_index: usize,
    },
    SourceFailed {
        source_id: String,
    },
    ObjectSynced {
        bucket: String,
        key: String,
    },
    PeerStarted {
        endpoint: String,
    },
    PeerStopped,
    PeerAnnounced {
        endpoint: String,
        fragments: Vec<usize>,
    },
    PeerFragmentRequested {
        fragment_index: usize,
    },
    PeerFragmentServed {
        fragment_index: usize,
    },
    PeerFragmentReceived {
        fragment_index: usize,
    },
    PeerFragmentValidated {
        fragment_index: usize,
    },
    PeerFragmentRejected {
        fragment_index: usize,
        reason: String,
    },
    PeerUnavailable {
        source_id: String,
    },
    PeerFallbackActivated {
        fragment_index: usize,
    },
}
