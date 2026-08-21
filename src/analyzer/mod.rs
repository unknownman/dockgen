pub mod dependencies;
pub mod env;
pub mod version;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::Language;

pub use self::dependencies::ManifestInfo;
#[allow(dead_code, unused_imports)]
pub use self::env::{ConnectionMatch, InfraScanResult};

// ---------------------------------------------------------------------------
// DirectoryAnalysis
// ---------------------------------------------------------------------------

/// Aggregated analysis results for a single directory context, produced by
/// the unified [`analyze_directory`] call.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryAnalysis {
    /// Merged manifest metadata (dependencies, dev-dependencies, scripts, etc.).
    pub manifest: ManifestInfo,
    /// Extracted runtime version string (e.g. `"20"`, `"3.11"`, `"1.78"`), if found.
    pub runtime_version: Option<String>,
    /// Merged environment variables from `.env` variants (highest priority wins).
    pub env_vars: BTreeMap<String, String>,
    /// Infrastructure connection strings and keys detected in environment variables.
    pub infra_scan: InfraScanResult,
}

// ---------------------------------------------------------------------------
// Facade functions
// ---------------------------------------------------------------------------

/// Orchestrates full directory analysis in a single unified call.
///
/// Calls each sub-analyzer (manifests, version, env files, infra detection)
/// and bundles the results into a [`DirectoryAnalysis`]. Avoids double
/// parsing of `.env` files by reusing the parsed `env_vars` map for
/// `detect_infra_connections`.
#[allow(dead_code)]
pub fn analyze_directory(dir_path: &Path, language: &Language) -> DirectoryAnalysis {
    let manifest = analyze_manifests(dir_path);
    let runtime_version = extract_version(dir_path, language);
    let env_vars = analyze_env_files(dir_path);
    let infra_scan = env::detect_infra_connections(&env_vars);

    DirectoryAnalysis {
        manifest,
        runtime_version,
        env_vars,
        infra_scan,
    }
}

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
/// Priority (highest wins): `.env.local` > `.env` > `.env.development`
/// > `.env.staging` > `.env.production` > `.env.example`.
pub fn analyze_env_files(dir_path: &Path) -> BTreeMap<String, String> {
    env::parse_env_files(dir_path)
}

/// Parses `.env` files in `dir_path` and detects infrastructure connection
/// strings (URLs, well-known keys).
///
/// Returns an [`InfraScanResult`] containing URL-pattern matches and
/// well-known infrastructure variable names found in the environment.
pub fn scan_project_env_infra(dir_path: &Path) -> InfraScanResult {
    let env = env::parse_env_files(dir_path);
    env::detect_infra_connections(&env)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_file(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).expect("failed to create test file");
    }

    // -- analyze_manifests ---------------------------------------------------

    #[test]
    fn analyze_manifests_package_json() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(
            root,
            "package.json",
            r#"{"name":"my-api","dependencies":{"express":"^4.18.0"},"devDependencies":{"jest":"^29.0"}}"#,
        );

        let info = analyze_manifests(root);
        assert_eq!(info.package_name.as_deref(), Some("my-api"));
        assert!(info.dependencies.contains(&"express".to_string()));
        assert!(info.dev_dependencies.contains(&"jest".to_string()));
    }

    #[test]
    fn analyze_manifests_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let info = analyze_manifests(tmp.path());
        assert!(info.package_name.is_none());
        assert!(info.dependencies.is_empty());
    }

    // -- extract_version -----------------------------------------------------

    #[test]
    fn extract_version_node_nvmrc() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".nvmrc", "20.11.0\n");

        let version = extract_version(tmp.path(), &Language::NodeJs);
        assert_eq!(version.as_deref(), Some("20"));
    }

    #[test]
    fn extract_version_python_version_file() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".python-version", "3.12.1\n");

        let version = extract_version(tmp.path(), &Language::Python);
        assert_eq!(version.as_deref(), Some("3.12"));
    }

    #[test]
    fn extract_version_no_file_returns_none() {
        let tmp = TempDir::new().unwrap();
        let version = extract_version(tmp.path(), &Language::NodeJs);
        assert!(version.is_none());
    }

    #[test]
    fn extract_version_unknown_language_returns_none() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".nvmrc", "20\n");
        let version = extract_version(tmp.path(), &Language::Unknown("zig".into()));
        assert!(version.is_none());
    }

    // -- analyze_env_files ---------------------------------------------------

    #[test]
    fn analyze_env_files_priority_local_overrides_env() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, ".env", "FOO=base\nBAR=from_env\n");
        create_file(root, ".env.local", "FOO=local_override\n");

        let env = analyze_env_files(root);
        assert_eq!(env.get("FOO").map(|s| s.as_str()), Some("local_override"));
        assert_eq!(env.get("BAR").map(|s| s.as_str()), Some("from_env"));
    }

    #[test]
    fn analyze_env_files_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let env = analyze_env_files(tmp.path());
        assert!(env.is_empty());
    }

    #[test]
    fn analyze_env_files_deterministic_order() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, ".env", "Z=1\nA=2\nM=3\n");

        let env = analyze_env_files(root);
        let keys: Vec<&String> = env.keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    // -- scan_project_env_infra ----------------------------------------------

    #[test]
    fn scan_project_env_infra_detects_postgres() {
        let tmp = TempDir::new().unwrap();
        create_file(
            tmp.path(),
            ".env",
            "DATABASE_URL=postgres://user:pass@localhost:5432/mydb\n",
        );

        let result = scan_project_env_infra(tmp.path());
        assert!(
            !result.url_matches.is_empty(),
            "expected at least one URL match"
        );

        let pg = result
            .url_matches
            .iter()
            .find(|m| m.variable == "DATABASE_URL");
        assert!(pg.is_some(), "expected DATABASE_URL match");
        assert_eq!(pg.unwrap().scheme, "postgres://");
    }

    #[test]
    fn scan_project_env_infra_known_keys() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".env", "REDIS_HOST=localhost\n");

        let result = scan_project_env_infra(tmp.path());
        assert!(
            result.known_infra_keys.contains(&"REDIS_HOST".to_string()),
            "expected REDIS_HOST in known keys"
        );
    }

    #[test]
    fn scan_project_env_infra_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let result = scan_project_env_infra(tmp.path());
        assert!(result.url_matches.is_empty());
        assert!(result.known_infra_keys.is_empty());
    }

    // -- analyze_directory ---------------------------------------------------

    #[test]
    fn analyze_directory_full_project() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(
            root,
            "package.json",
            r#"{"name":"full-app","scripts":{"start":"node index.js"}}"#,
        );
        create_file(root, "index.js", "console.log('hello');\n");
        create_file(root, ".nvmrc", "20\n");
        create_file(root, ".env", "DATABASE_URL=postgres://localhost/app\n");

        let analysis = analyze_directory(root, &Language::NodeJs);

        // Manifest.
        assert_eq!(analysis.manifest.package_name.as_deref(), Some("full-app"));

        // Version.
        assert_eq!(analysis.runtime_version.as_deref(), Some("20"));

        // Env vars.
        assert!(analysis.env_vars.contains_key("DATABASE_URL"));

        // Infra scan.
        assert!(!analysis.infra_scan.url_matches.is_empty());
        assert!(analysis
            .infra_scan
            .url_matches
            .iter()
            .any(|m| m.variable == "DATABASE_URL"));
    }

    #[test]
    fn analyze_directory_empty_dir_returns_safe_defaults() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let analysis = analyze_directory(root, &Language::NodeJs);

        assert!(analysis.manifest.package_name.is_none());
        assert!(analysis.manifest.dependencies.is_empty());
        assert!(analysis.runtime_version.is_none());
        assert!(analysis.env_vars.is_empty());
        assert!(analysis.infra_scan.url_matches.is_empty());
        assert!(analysis.infra_scan.known_infra_keys.is_empty());
    }

    #[test]
    fn analyze_directory_reuses_env_for_infra() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(
            root,
            ".env",
            "DATABASE_URL=postgres://localhost/app\nREDIS_HOST=localhost\n",
        );

        let analysis = analyze_directory(root, &Language::NodeJs);

        // Env vars should contain both keys.
        assert!(analysis.env_vars.contains_key("DATABASE_URL"));
        assert!(analysis.env_vars.contains_key("REDIS_HOST"));

        // Infra scan should detect the postgres URL.
        assert!(analysis
            .infra_scan
            .url_matches
            .iter()
            .any(|m| m.variable == "DATABASE_URL"));

        // Infra scan should include the known key.
        assert!(analysis
            .infra_scan
            .known_infra_keys
            .contains(&"REDIS_HOST".to_string()));
    }

    // -- DirectoryAnalysis serde roundtrip ------------------------------------

    #[test]
    fn directory_analysis_serde_roundtrip() {
        let mut env_vars = BTreeMap::new();
        env_vars.insert("DATABASE_URL".into(), "postgres://localhost/app".into());

        let analysis = DirectoryAnalysis {
            manifest: ManifestInfo {
                package_name: Some("test-app".into()),
                dependencies: vec!["express".into()],
                dev_dependencies: vec!["jest".into()],
                scripts: std::collections::HashMap::from([(
                    "start".into(),
                    "node index.js".into(),
                )]),
                entrypoint: Some("index.js".into()),
                raw_content: std::collections::HashMap::new(),
            },
            runtime_version: Some("20".into()),
            env_vars,
            infra_scan: InfraScanResult {
                url_matches: vec![ConnectionMatch {
                    variable: "DATABASE_URL".into(),
                    scheme: "postgres://".into(),
                    value: "postgres://localhost/app".into(),
                }],
                known_infra_keys: vec![],
            },
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: DirectoryAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(analysis, deserialized);
    }
}
