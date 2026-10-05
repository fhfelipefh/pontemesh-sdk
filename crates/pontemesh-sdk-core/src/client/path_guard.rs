use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::errors::PontemeshError;

/// Validates that a target path resides strictly within one of the allowed directories.
///
/// If `allowed_directories` is empty, path containment is not enforced, but basic sanity checks
/// (such as rejecting null bytes) are still executed.
/// If `allowed_directories` is non-empty, any path that escapes or is located outside
/// will be rejected immediately with [`PontemeshError::PathNotAllowed`].
pub fn validate_path_against_allowed(
    target: &Path,
    allowed_directories: &[PathBuf],
) -> Result<PathBuf, PontemeshError> {
    let path_str = target.to_string_lossy();
    if path_str.contains('\0') {
        return Err(PontemeshError::InvalidArgument(
            "destination path contains null bytes".to_string(),
        ));
    }

    if allowed_directories.is_empty() {
        return Ok(target.to_path_buf());
    }

    let canonical_dest = resolve_canonical_destination(target)?;

    let mut is_allowed = false;
    for allowed in allowed_directories {
        let canonical_allowed = resolve_canonical_directory(allowed)?;
        if is_path_contained_in(&canonical_dest, &canonical_allowed) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err(PontemeshError::PathNotAllowed(target.to_path_buf()));
    }

    Ok(canonical_dest)
}

fn resolve_canonical_destination(target: &Path) -> Result<PathBuf, PontemeshError> {
    if target.exists() {
        return Ok(fs::canonicalize(target)?);
    }

    let mut non_existing_components = Vec::new();
    let mut current = target;

    while !current.exists() {
        if let Some(file_name) = current.file_name() {
            non_existing_components.push(file_name);
            if let Some(parent) = current.parent() {
                if parent.as_os_str().is_empty() {
                    current = Path::new(".");
                    break;
                }
                current = parent;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if !current.exists() {
        return Err(PontemeshError::PathNotAllowed(target.to_path_buf()));
    }

    let mut canonical = fs::canonicalize(current)?;
    non_existing_components.reverse();

    for comp in non_existing_components {
        let comp_path = Path::new(comp);
        for c in comp_path.components() {
            match c {
                Component::Normal(n) => canonical.push(n),
                Component::CurDir => continue,
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(PontemeshError::PathNotAllowed(target.to_path_buf()));
                }
            }
        }
    }

    Ok(canonical)
}

fn resolve_canonical_directory(dir: &Path) -> Result<PathBuf, PontemeshError> {
    if dir.exists() {
        Ok(fs::canonicalize(dir)?)
    } else {
        resolve_canonical_destination(dir)
    }
}

pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{stripped}"));
        }
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

pub fn is_path_contained_in(target: &Path, base: &Path) -> bool {
    let clean_target = strip_verbatim_prefix(target);
    let clean_base = strip_verbatim_prefix(base);

    let target_components: Vec<_> = clean_target.components().collect();
    let base_components: Vec<_> = clean_base.components().collect();

    if base_components.len() > target_components.len() {
        return false;
    }

    for (b, t) in base_components.iter().zip(target_components.iter()) {
        #[cfg(windows)]
        {
            let b_str = b.as_os_str().to_string_lossy();
            let t_str = t.as_os_str().to_string_lossy();
            if !b_str.eq_ignore_ascii_case(&t_str) {
                return false;
            }
        }
        #[cfg(not(windows))]
        {
            if b != t {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_destinations_inside_allowed_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed = temp.path().join("game");
        fs::create_dir_all(&allowed).expect("create allowed");

        let dest = allowed.join("bin").join("game.exe");
        let result = validate_path_against_allowed(&dest, std::slice::from_ref(&allowed));
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_destinations_outside_allowed_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed = temp.path().join("game");
        let outside = temp.path().join("system_outside");
        fs::create_dir_all(&allowed).expect("create allowed");
        fs::create_dir_all(&outside).expect("create outside");

        let dest = outside.join("backdoor.exe");
        let result = validate_path_against_allowed(&dest, &[allowed]);
        assert!(matches!(result, Err(PontemeshError::PathNotAllowed(_))));
    }

    #[test]
    fn rejects_path_traversal_attempts_escaping_allowed_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed = temp.path().join("game");
        fs::create_dir_all(&allowed).expect("create allowed");

        let traversal = allowed.join("..").join("outside.exe");
        let result = validate_path_against_allowed(&traversal, &[allowed]);
        assert!(matches!(result, Err(PontemeshError::PathNotAllowed(_))));
    }

    #[test]
    fn rejects_prefix_confusion_attack() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed = temp.path().join("game");
        let evil = temp.path().join("game_evil");
        fs::create_dir_all(&allowed).expect("create allowed");
        fs::create_dir_all(&evil).expect("create evil");

        let dest = evil.join("exploit.bin");
        let result = validate_path_against_allowed(&dest, &[allowed]);
        assert!(matches!(result, Err(PontemeshError::PathNotAllowed(_))));
    }

    #[test]
    fn rejects_null_byte_in_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed = temp.path().join("game");
        fs::create_dir_all(&allowed).expect("create allowed");

        let dest = PathBuf::from(format!("{}/game.exe\0.dll", allowed.display()));
        let result = validate_path_against_allowed(&dest, &[allowed]);
        assert!(matches!(result, Err(PontemeshError::InvalidArgument(_))));
    }

    #[test]
    fn supports_multiple_allowed_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let allowed1 = temp.path().join("game");
        let allowed2 = temp.path().join("mods");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&allowed1).expect("create allowed1");
        fs::create_dir_all(&allowed2).expect("create allowed2");
        fs::create_dir_all(&outside).expect("create outside");

        let dirs = vec![allowed1.clone(), allowed2.clone()];

        assert!(validate_path_against_allowed(&allowed1.join("file1.bin"), &dirs).is_ok());
        assert!(validate_path_against_allowed(&allowed2.join("mod.pak"), &dirs).is_ok());
        assert!(matches!(
            validate_path_against_allowed(&outside.join("bad.exe"), &dirs),
            Err(PontemeshError::PathNotAllowed(_))
        ));
    }
}
