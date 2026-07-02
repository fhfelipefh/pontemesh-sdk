use crate::errors::PontemeshError;

pub fn should_fallback(error: &PontemeshError) -> bool {
    !matches!(
        error,
        PontemeshError::Cancelled | PontemeshError::AccessDenied(_)
    )
}
