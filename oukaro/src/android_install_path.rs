use std::path::{Path, PathBuf};

pub fn has_known_user_app_prefix(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");

    normalized.starts_with("/data/app/")
        || normalized.starts_with("/data/app-private/")
        || is_adopted_storage_app_path(&normalized)
}

fn is_known_existing_user_app_path(path: &Path) -> bool {
    has_known_user_app_prefix(path) || { false }
}
pub fn normalize_user_app_code_path(path: &Path) -> Option<PathBuf> {
    if path.is_dir() && is_known_existing_user_app_path(path) {
        return Some(path.to_path_buf());
    }

    let parent = if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"))
        || path.is_file()
    {
        path.parent()?
    } else {
        return None;
    };

    if parent.exists() && is_known_existing_user_app_path(parent) {
        return Some(parent.to_path_buf());
    }

    None
}
fn is_adopted_storage_app_path(normalized: &str) -> bool {
    let Some(rest) = normalized.strip_prefix("/mnt/expand/") else {
        return false;
    };

    let mut parts = rest.split('/');
    matches!(
        (parts.next(), parts.next()),
        (Some(uuid), Some("app")) if !uuid.is_empty()
    )
}
