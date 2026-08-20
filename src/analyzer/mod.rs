pub mod dependencies;
pub mod version;

use std::path::Path;

use crate::models::Language;

use self::dependencies::ManifestInfo;

/// Parses all detectable manifests in `dir_path` and returns a merged
/// [`ManifestInfo`].
pub fn analyze_manifests(dir_path: &Path) -> ManifestInfo {
    dependencies::parse_directory_manifests(dir_path)
}

/// Extracts the runtime version for the given `language` from files in
/// `dir_path`.
pub fn extract_version(dir_path: &Path, language: &Language) -> Option<String> {
    version::extract_runtime_version(dir_path, language)
}
