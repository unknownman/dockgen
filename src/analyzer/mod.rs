pub mod dependencies;
pub mod env;
pub mod version;

use std::collections::BTreeMap;
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

/// Scans for `.env` files in `dir_path` and returns a merged, deduplicated
/// map of environment variables.
///
/// Priority: `.env` > `.env.local` > `.env.development` > `.env.staging`
/// > `.env.production` > `.env.example`.
pub fn analyze_env_files(dir_path: &Path) -> BTreeMap<String, String> {
    env::parse_env_files(dir_path)
}
