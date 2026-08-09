use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentProgressState {
    Pending,
    Downloading,
    Validated,
    Failed,
    Invalid,
    Fallback,
    Shareable,
}

#[derive(Default)]
pub struct ProgressMap {
    bytes_by_fragment: HashMap<usize, u64>,
    state_by_fragment: HashMap<usize, FragmentProgressState>,
}

impl ProgressMap {
    pub fn mark(&mut self, fragment_index: usize, bytes_downloaded: u64) {
        self.bytes_by_fragment
            .insert(fragment_index, bytes_downloaded);
        self.state_by_fragment
            .insert(fragment_index, FragmentProgressState::Validated);
    }

    pub fn mark_state(&mut self, fragment_index: usize, state: FragmentProgressState) {
        self.state_by_fragment.insert(fragment_index, state);
    }

    pub fn bytes_downloaded(&self, fragment_index: usize) -> u64 {
        self.bytes_by_fragment
            .get(&fragment_index)
            .copied()
            .unwrap_or_default()
    }

    pub fn total_bytes_downloaded(&self) -> u64 {
        self.bytes_by_fragment.values().sum()
    }

    pub fn state(&self, fragment_index: usize) -> FragmentProgressState {
        self.state_by_fragment
            .get(&fragment_index)
            .copied()
            .unwrap_or(FragmentProgressState::Pending)
    }
}
