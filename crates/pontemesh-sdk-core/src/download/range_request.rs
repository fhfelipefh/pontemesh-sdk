use crate::contracts::FragmentDescriptor;

pub fn range_header(fragment: &FragmentDescriptor) -> String {
    fragment.fallback_range_header.clone()
}
