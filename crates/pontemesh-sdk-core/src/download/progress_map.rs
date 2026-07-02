use std::collections::HashMap;

#[derive(Default)]
pub struct ProgressMap {
    bytes_by_fragment: HashMap<usize, u64>,
}

impl ProgressMap {
    pub fn mark(&mut self, fragment_index: usize, bytes_downloaded: u64) {
        self.bytes_by_fragment
            .insert(fragment_index, bytes_downloaded);
    }

    pub fn bytes_downloaded(&self, fragment_index: usize) -> u64 {
        self.bytes_by_fragment
            .get(&fragment_index)
            .copied()
            .unwrap_or_default()
    }
}
