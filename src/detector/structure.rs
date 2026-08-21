use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{ServiceType, EXCLUDED_DIRS};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Manifest file names that indicate a service root.
const MANIFEST_FILES: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "requirements.txt",
    "pyproject.toml",
    "Pipfile",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "Gemfile",
];

/// Well-known directory names that map to a specific [`ServiceType`].
const FRONTEND_DIRS: &[&str] = &["frontend", "web", "client", "ui", "app"];
const BACKEND_DIRS: &[&str] = &["backend", "server", "api", "gateway"];
const CONTAINER_DIRS: &[&str] = &["services", "apps", "packages"];

/// Source file extensions used to verify that a candidate directory contains
/// actual code, not just manifests.
const SOURCE_FILE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "kt", "php", "cs", "fs", "rb",
];

/// Maximum depth (from `dir`) to scan for source files when verifying a
/// candidate.
const SOURCE_FILE_SCAN_DEPTH: usize = 3;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A raw service candidate discovered during structural analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredServiceCandidate {
    /// Human-readable name (derived from the directory name).
    pub name: String,
    /// Path relative to the project root.
    pub relative_path: PathBuf,
    /// Absolute path on disk.
    pub full_path: PathBuf,
    /// Assigned service type based on naming conventions.
    pub service_type: ServiceType,
    /// Manifest files found at the service root.
    pub manifest_files: Vec<PathBuf>,
}

/// Result of workspace and multi-service structural analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStructure {
    /// Absolute path to the project root.
    pub root_path: PathBuf,
    /// Whether the project is a monorepo / multi-service workspace.
    pub is_monorepo: bool,
    /// Detected workspace orchestration tool, if any.
    pub workspace_tool: Option<String>,
    /// Discovered service candidates, sorted by `relative_path`.
    pub candidates: Vec<DiscoveredServiceCandidate>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyses the directory structure at `root_path` and returns a
/// [`WorkspaceStructure`] describing discovered services.
///
/// # Errors
///
/// Returns an `io::Error` if the root path cannot be read.
pub fn analyze_structure(root_path: &Path) -> Result<WorkspaceStructure, std::io::Error> {
    let root_path = root_path.to_path_buf();
    let workspace_tool = detect_workspace_tool(&root_path);
    let is_monorepo = workspace_tool.is_some();

    let mut candidates = if is_monorepo {
        discover_monorepo_candidates(&root_path)
    } else {
        discover_flat_candidates(&root_path)
    };

    // Deterministic ordering.
    candidates.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(WorkspaceStructure {
        root_path,
        is_monorepo,
        workspace_tool,
        candidates,
    })
}

// ---------------------------------------------------------------------------
// Level 1 — Workspace tool detection
// ---------------------------------------------------------------------------

/// Scans the root directory for known workspace orchestrator configuration
/// files and returns the tool name if found.
pub fn detect_workspace_tool(root: &Path) -> Option<String> {
    // Dedicated config files (checked first — unambiguous).
    if root.join("turbo.json").is_file() {
        return Some("turborepo".to_string());
    }
    if root.join("nx.json").is_file() {
        return Some("nx".to_string());
    }
    if root.join("pnpm-workspace.yaml").is_file() {
        return Some("pnpm".to_string());
    }
    if root.join("lerna.json").is_file() {
        return Some("lerna".to_string());
    }
    if root.join("go.work").is_file() {
        return Some("go-work".to_string());
    }

    // Cargo workspace — requires reading and parsing Cargo.toml.
    if let Some(tool) = detect_cargo_workspace(root) {
        return Some(tool);
    }

    // npm/yarn/bun workspaces — requires reading package.json.
    if let Some(tool) = detect_npm_workspace(root) {
        return Some(tool);
    }

    None
}

/// Checks whether `Cargo.toml` at `root` defines a `[workspace]`.
fn detect_cargo_workspace(root: &Path) -> Option<String> {
    let cargo_path = root.join("Cargo.toml");
    if !cargo_path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&cargo_path).ok()?;
    let table: toml::Value = toml::from_str(&content).ok()?;
    if table.get("workspace").is_some() {
        Some("cargo".to_string())
    } else {
        None
    }
}

/// Checks whether `package.json` at `root` contains a `"workspaces"` field.
/// Disambiguates the workspace tool by checking for lockfiles.
fn detect_npm_workspace(root: &Path) -> Option<String> {
    let pkg_path = root.join("package.json");
    if !pkg_path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&pkg_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    if value.get("workspaces").is_some() {
        // Disambiguate by lockfile presence.
        if root.join("pnpm-lock.yaml").is_file() {
            Some("pnpm".to_string())
        } else if root.join("yarn.lock").is_file() {
            Some("yarn".to_string())
        } else if root.join("bun.lockb").is_file() || root.join("bun.lock").is_file() {
            Some("bun".to_string())
        } else {
            Some("npm".to_string())
        }
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Level 2 — Structural discovery
// ---------------------------------------------------------------------------

/// Discovers service candidates in a monorepo by scanning well-known
/// container directories (`apps/`, `services/`, `packages/`) and direct
/// children.
fn discover_monorepo_candidates(root: &Path) -> Vec<DiscoveredServiceCandidate> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();

    // 1. Scan well-known container directories.
    for container_dir in CONTAINER_DIRS {
        let container_path = root.join(container_dir);
        if !container_path.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&container_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = file_name(&path);
                if is_excluded(&dir_name) {
                    continue;
                }
                if seen.contains(&path) {
                    continue;
                }

                let manifests = find_manifests(&path);
                if !manifests.is_empty() && has_verifiable_source_files(&path) {
                    let relative = relative_path(root, &path);
                    candidates.push(DiscoveredServiceCandidate {
                        name: dir_name.clone(),
                        relative_path: relative,
                        full_path: path.clone(),
                        service_type: ServiceType::MonorepoMember,
                        manifest_files: manifests,
                    });
                    seen.insert(path);
                }
            }
        }
    }

    // 2. Scan direct children for well-named service dirs.
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = file_name(&path);
            if is_excluded(&dir_name) {
                continue;
            }
            if seen.contains(&path) {
                continue;
            }

            let manifests = find_manifests(&path);
            if !manifests.is_empty() && has_verifiable_source_files(&path) {
                let st = classify_service_type(&dir_name);
                let relative = relative_path(root, &path);
                candidates.push(DiscoveredServiceCandidate {
                    name: dir_name,
                    relative_path: relative,
                    full_path: path.clone(),
                    service_type: st,
                    manifest_files: manifests,
                });
                seen.insert(path);
            }
        }
    }

    candidates
}

/// Discovers service candidates in a flat (non-monorepo) project.
///
/// - If depth-1 subdirectories contain well-known service names
///   (`frontend`, `backend`, `api`, etc.) **and** their own manifests, they are
///   returned as distinct candidates (e.g. a monorepo-like layout without a
///   workspace tool).
/// - Otherwise, if the root itself contains manifests, returns a single `Single`
///   candidate.
/// - Otherwise, scans depth-1 subdirectories for any manifests.
fn discover_flat_candidates(root: &Path) -> Vec<DiscoveredServiceCandidate> {
    // Check for well-known service sub-directories first — even when the root
    // has manifests (polyglot monorepo layout without workspace tool).
    let sub_services = discover_well_known_sub_services(root);
    if !sub_services.is_empty() {
        return sub_services;
    }

    let root_manifests = find_manifests(root);
    if !root_manifests.is_empty() {
        return vec![DiscoveredServiceCandidate {
            name: file_name(root),
            relative_path: PathBuf::from("."),
            full_path: root.to_path_buf(),
            service_type: ServiceType::Single,
            manifest_files: root_manifests,
        }];
    }

    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = file_name(&path);
            if is_excluded(&dir_name) {
                continue;
            }
            let manifests = find_manifests(&path);
            if !manifests.is_empty() && has_verifiable_source_files(&path) {
                let st = classify_service_type(&dir_name);
                let relative = relative_path(root, &path);
                candidates.push(DiscoveredServiceCandidate {
                    name: dir_name,
                    relative_path: relative,
                    full_path: path.clone(),
                    service_type: st,
                    manifest_files: manifests,
                });
            }
        }
    }

    candidates
}

/// Scans depth-1 subdirectories for well-known service names that contain
/// their own manifests.
fn discover_well_known_sub_services(root: &Path) -> Vec<DiscoveredServiceCandidate> {
    let mut candidates = Vec::new();
    let all_well_known: Vec<&str> = FRONTEND_DIRS
        .iter()
        .chain(BACKEND_DIRS.iter())
        .copied()
        .collect();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = file_name(&path);
            if is_excluded(&dir_name) {
                continue;
            }
            let lower = dir_name.to_ascii_lowercase();
            if !all_well_known.contains(&lower.as_str()) {
                continue;
            }
            let manifests = find_manifests(&path);
            if !manifests.is_empty() && has_verifiable_source_files(&path) {
                let st = classify_service_type(&dir_name);
                let relative = relative_path(root, &path);
                candidates.push(DiscoveredServiceCandidate {
                    name: dir_name,
                    relative_path: relative,
                    full_path: path,
                    service_type: st,
                    manifest_files: manifests,
                });
            }
        }
    }

    candidates
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `dir_name` is in the exclusion list.
pub fn is_excluded(dir_name: &str) -> bool {
    EXCLUDED_DIRS.contains(&dir_name)
}

/// Returns `true` if `dir` (non-recursively, up to `SOURCE_FILE_SCAN_DEPTH`
/// levels deep) contains at least one file whose extension is in
/// `SOURCE_FILE_EXTENSIONS`.
///
/// This prevents manifest-only directories (e.g. a `packages/shared`
/// containing only a `package.json` with no source files) from being
/// treated as distinct deployable services.
pub fn has_verifiable_source_files(dir: &Path) -> bool {
    has_source_files_recursive(dir, 0)
}

/// Recursive helper for `has_verifiable_source_files`.
fn has_source_files_recursive(dir: &Path, depth: usize) -> bool {
    if depth > SOURCE_FILE_SCAN_DEPTH {
        return false;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SOURCE_FILE_EXTENSIONS.contains(&ext) {
                    return true;
                }
            }
        } else if path.is_dir() {
            let dir_name = file_name(&path);
            if !is_excluded(&dir_name) && has_source_files_recursive(&path, depth + 1) {
                return true;
            }
        }
    }
    false
}

/// Scans `dir` (non-recursively) for known manifest file names and returns
/// their relative paths.
fn find_manifests(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for name in MANIFEST_FILES {
        let p = dir.join(name);
        if p.is_file() {
            found.push(PathBuf::from(name));
        }
    }
    found
}

/// Classifies a directory name into a [`ServiceType`].
fn classify_service_type(dir_name: &str) -> ServiceType {
    let lower = dir_name.to_ascii_lowercase();
    if FRONTEND_DIRS.contains(&lower.as_str()) {
        return ServiceType::Frontend;
    }
    if BACKEND_DIRS.contains(&lower.as_str()) {
        return ServiceType::Backend;
    }
    // Subdirectories inside well-known containers are MonorepoMember — but
    // this is handled at the call-site. Here we just handle top-level names.
    ServiceType::MonorepoMember
}

/// Extracts the file / directory name as a `String`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Computes `path` relative to `base`.
fn relative_path(base: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(base).unwrap_or(path).to_path_buf()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // -- helpers -----------------------------------------------------------

    fn create_file(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").expect("failed to create test file");
    }

    fn create_dir(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(&p).expect("failed to create test dir");
        p
    }

    // -- is_excluded -------------------------------------------------------

    #[test]
    fn excluded_dirs_recognized() {
        assert!(is_excluded("node_modules"));
        assert!(is_excluded(".git"));
        assert!(is_excluded("target"));
        assert!(is_excluded("vendor"));
        assert!(is_excluded("__pycache__"));
        assert!(is_excluded(".venv"));
    }

    #[test]
    fn non_excluded_dir_not_flagged() {
        assert!(!is_excluded("src"));
        assert!(!is_excluded("frontend"));
        assert!(!is_excluded("backend"));
    }

    // -- single service root -----------------------------------------------

    #[test]
    fn single_service_root_detection() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "package.json");

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert!(result.workspace_tool.is_none());
        assert_eq!(result.candidates.len(), 1);

        let svc = &result.candidates[0];
        assert_eq!(
            svc.name,
            root.file_name().unwrap().to_string_lossy().as_ref()
        );
        assert_eq!(svc.service_type, ServiceType::Single);
        assert_eq!(svc.relative_path, PathBuf::from("."));
        assert!(svc.manifest_files.contains(&PathBuf::from("package.json")));
    }

    #[test]
    fn single_service_cargo_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "Cargo.toml");

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].service_type, ServiceType::Single);
    }

    // -- monorepo detection (Level 1) --------------------------------------

    #[test]
    fn turbo_json_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");
        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("turborepo"));
    }

    #[test]
    fn nx_json_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "nx.json");
        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("nx"));
    }

    #[test]
    fn pnpm_workspace_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "pnpm-workspace.yaml");
        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("pnpm"));
    }

    #[test]
    fn lerna_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "lerna.json");
        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("lerna"));
    }

    #[test]
    fn go_work_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "go.work");
        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("go-work"));
    }

    #[test]
    fn cargo_workspace_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "Cargo.toml");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();

        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("cargo"));
    }

    #[test]
    fn npm_workspace_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(
            root.join("package.json"),
            r#"{ "workspaces": ["apps/*", "packages/*"] }"#,
        )
        .unwrap();

        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.workspace_tool.as_deref(), Some("npm"));
    }

    #[test]
    fn non_workspace_cargo_toml_not_monorepo() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // A Cargo.toml without [workspace] should not trigger monorepo.
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert!(result.workspace_tool.is_none());
    }

    #[test]
    fn non_workspace_package_json_not_monorepo() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(
            root.join("package.json"),
            r#"{ "name": "foo", "version": "1.0.0" }"#,
        )
        .unwrap();

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert!(result.workspace_tool.is_none());
    }

    // -- empirical discovery (Level 2) -------------------------------------

    #[test]
    fn frontend_backend_folders_detected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Set up a monorepo so sub-dirs are scanned.
        create_file(root, "turbo.json");

        // frontend
        let fe = create_dir(root, "frontend");
        create_file(&fe, "package.json");
        create_file(&fe, "index.ts"); // source file

        // backend
        let be = create_dir(root, "backend");
        create_file(&be, "package.json");
        create_file(&be, "server.js"); // source file

        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert_eq!(result.candidates.len(), 2);

        let names: Vec<&str> = result.candidates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"backend"));
        assert!(names.contains(&"frontend"));

        let fe_candidate = result
            .candidates
            .iter()
            .find(|c| c.name == "frontend")
            .unwrap();
        assert_eq!(fe_candidate.service_type, ServiceType::Frontend);

        let be_candidate = result
            .candidates
            .iter()
            .find(|c| c.name == "backend")
            .unwrap();
        assert_eq!(be_candidate.service_type, ServiceType::Backend);
    }

    #[test]
    fn well_known_container_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        // apps/web
        let apps_web = create_dir(root, "apps");
        let apps_web = create_dir(&apps_web, "web");
        create_file(&apps_web, "package.json");
        create_file(&apps_web, "index.ts"); // source file

        // services/api
        let svc_api = create_dir(root, "services");
        let svc_api = create_dir(&svc_api, "api");
        create_file(&svc_api, "go.mod");
        create_file(&svc_api, "main.go"); // source file

        // packages/shared
        let pkg = create_dir(root, "packages");
        let pkg = create_dir(&pkg, "shared");
        create_file(&pkg, "package.json");
        create_file(&pkg, "index.ts"); // source file

        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert!(result.candidates.len() >= 3);

        let paths: Vec<PathBuf> = result
            .candidates
            .iter()
            .map(|c| c.relative_path.clone())
            .collect();
        assert!(paths.contains(&PathBuf::from("apps/web")));
        assert!(paths.contains(&PathBuf::from("services/api")));
        assert!(paths.contains(&PathBuf::from("packages/shared")));
    }

    #[test]
    fn node_modules_excluded() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        // node_modules should be skipped even if it contains package.json.
        let nm = create_dir(root, "node_modules");
        create_file(&nm, "package.json");

        // A real service.
        let svc = create_dir(root, "api");
        create_file(&svc, "package.json");
        create_file(&svc, "main.ts"); // source file

        let result = analyze_structure(root).unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "api");
    }

    #[test]
    fn git_dir_excluded() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        let git_dir = create_dir(root, ".git");
        create_file(&git_dir, "config");

        let svc = create_dir(root, "web");
        create_file(&svc, "package.json");
        create_file(&svc, "index.ts"); // source file

        let result = analyze_structure(root).unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "web");
    }

    #[test]
    fn dist_and_build_excluded() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        let dist = create_dir(root, "dist");
        create_file(&dist, "package.json");

        let build = create_dir(root, "build");
        create_file(&build, "Cargo.toml");

        let svc = create_dir(root, "app");
        create_file(&svc, "package.json");
        create_file(&svc, "index.ts"); // source file

        let result = analyze_structure(root).unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "app");
    }

    #[test]
    fn no_manifests_no_candidates() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        // Empty sub-dirs with no manifests.
        create_dir(root, "frontend");
        create_dir(root, "backend");

        let result = analyze_structure(root).unwrap();

        assert!(result.is_monorepo);
        assert!(result.candidates.is_empty());
    }

    // -- flat project with sub-dirs ----------------------------------------

    #[test]
    fn flat_project_with_sub_services() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // No monorepo tool, root has no manifest.
        let fe = create_dir(root, "frontend");
        create_file(&fe, "package.json");
        create_file(&fe, "App.tsx"); // source file

        let be = create_dir(root, "backend");
        create_file(&be, "go.mod");
        create_file(&be, "main.go"); // source file

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert!(result.workspace_tool.is_none());
        assert_eq!(result.candidates.len(), 2);

        let fe_c = result
            .candidates
            .iter()
            .find(|c| c.name == "frontend")
            .unwrap();
        assert_eq!(fe_c.service_type, ServiceType::Frontend);

        let be_c = result
            .candidates
            .iter()
            .find(|c| c.name == "backend")
            .unwrap();
        assert_eq!(be_c.service_type, ServiceType::Backend);
    }

    #[test]
    fn root_manifest_with_well_known_sub_services() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Root has a manifest (e.g. Laravel + frontend build).
        create_file(root, "composer.json");
        create_file(root, "package.json");

        // Well-known sub-services with their own manifests.
        let fe = create_dir(root, "frontend");
        create_file(&fe, "package.json");
        create_file(&fe, "App.tsx"); // source file

        let be = create_dir(root, "backend");
        create_file(&be, "composer.json");
        create_file(&be, "app.php"); // source file

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        // Should discover sub-services, not root as Single.
        assert_eq!(result.candidates.len(), 2);

        let names: Vec<&str> = result.candidates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"frontend"));
        assert!(names.contains(&"backend"));

        let fe_c = result
            .candidates
            .iter()
            .find(|c| c.name == "frontend")
            .unwrap();
        assert_eq!(fe_c.service_type, ServiceType::Frontend);

        let be_c = result
            .candidates
            .iter()
            .find(|c| c.name == "backend")
            .unwrap();
        assert_eq!(be_c.service_type, ServiceType::Backend);
    }

    #[test]
    fn root_manifest_no_well_known_sub_services() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Root has manifest, sub-dirs are NOT well-known service names.
        create_file(root, "composer.json");

        let app = create_dir(root, "app");
        create_file(&app, "User.php");

        let config = create_dir(root, "config");
        create_file(&config, "app.php");

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].service_type, ServiceType::Single);
    }

    // -- deterministic sorting ----------------------------------------------

    #[test]
    fn candidates_sorted_by_relative_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        let c = create_dir(root, "services");
        let api = create_dir(&c, "api");
        create_file(&api, "package.json");
        create_file(&api, "main.ts"); // source file
        let web = create_dir(&c, "web");
        create_file(&web, "package.json");
        create_file(&web, "app.tsx"); // source file
        let auth = create_dir(&c, "auth");
        create_file(&auth, "go.mod");
        create_file(&auth, "main.go"); // source file

        let result = analyze_structure(root).unwrap();

        let paths: Vec<PathBuf> = result
            .candidates
            .iter()
            .map(|c| c.relative_path.clone())
            .collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }

    // -- multiple manifests -------------------------------------------------

    #[test]
    fn multiple_manifests_collected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        let svc = create_dir(root, "api");
        create_file(&svc, "Cargo.toml");
        create_file(&svc, "docker-compose.yml");
        create_file(&svc, "main.rs"); // source file

        let result = analyze_structure(root).unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert!(result.candidates[0]
            .manifest_files
            .contains(&PathBuf::from("Cargo.toml")));
    }

    // -- relative paths ----------------------------------------------------

    #[test]
    fn relative_paths_are_correct() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        let apps = create_dir(root, "apps");
        let web = create_dir(&apps, "web");
        create_file(&web, "package.json");
        create_file(&web, "index.ts"); // source file

        let result = analyze_structure(root).unwrap();

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(
            result.candidates[0].relative_path,
            PathBuf::from("apps/web")
        );
    }

    // -- source file verification -------------------------------------------

    #[test]
    fn monorepo_candidate_without_source_files_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        create_file(root, "turbo.json");

        // apps/web has a package.json but no source files.
        let apps_web = create_dir(root, "apps");
        let apps_web = create_dir(&apps_web, "web");
        create_file(&apps_web, "package.json");

        // services/api has a go.mod AND source files — should be discovered.
        let svc_api = create_dir(root, "services");
        let svc_api = create_dir(&svc_api, "api");
        create_file(&svc_api, "go.mod");
        create_file(&svc_api, "main.go");

        let result = analyze_structure(root).unwrap();

        // Only the api service should be discovered (web has no source files).
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "api");
    }

    #[test]
    fn flat_subdir_without_source_files_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // No monorepo tool, no root manifest — triggers flat candidate scan.
        let fe = create_dir(root, "frontend");
        create_file(&fe, "package.json");
        // No source files in frontend/ — should NOT be discovered.

        let be = create_dir(root, "backend");
        create_file(&be, "go.mod");
        create_file(&be, "main.go"); // Has source files — should be discovered.

        let result = analyze_structure(root).unwrap();

        assert!(!result.is_monorepo);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "backend");
    }

    #[test]
    fn has_verifiable_source_files_scans_depth_3() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create nested structure: src/components/Button.tsx
        let src = create_dir(root, "src");
        let components = create_dir(&src, "components");
        create_file(&components, "Button.tsx");

        assert!(has_verifiable_source_files(root));
    }

    #[test]
    fn has_verifiable_source_files_ignores_excluded_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Source file inside node_modules should NOT count.
        let nm = create_dir(root, "node_modules");
        create_file(&nm, "index.js");

        assert!(!has_verifiable_source_files(root));
    }

    #[test]
    fn has_verifiable_source_files_returns_false_for_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        create_dir(root, "empty");

        assert!(!has_verifiable_source_files(&root.join("empty")));
    }
}
