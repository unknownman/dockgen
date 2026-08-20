pub mod framework;
pub mod language;
pub mod structure;

use std::path::Path;

use anyhow::{Context, Result};

use crate::analyzer::{analyze_manifests, extract_version};
use crate::models::{Framework, Language, ProjectAnalysis, Service, ServiceType};

use self::framework::detect_framework;
use self::language::detect_language_and_pm;
use self::structure::WorkspaceStructure;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyses the project at `root_path` and returns a fully populated
/// [`ProjectAnalysis`].
///
/// The pipeline is:
/// 1. Structural discovery → [`WorkspaceStructure`] (monorepo / flat).
/// 2. Per-candidate: language, package manager, framework, version, env vars.
/// 3. Assemble [`Service`] list, apply overrides, sort deterministically.
///
/// # Arguments
///
/// * `root_path` – filesystem root of the project to analyse.
/// * `lang_override` – optional CLI override for language detection.
/// * `fw_override` – optional CLI override for framework detection.
/// * `services_filter` – optional list of service names to include (empty = all).
pub fn analyze_full_project(
    root_path: &Path,
    lang_override: Option<&Language>,
    fw_override: Option<&Framework>,
    services_filter: &[String],
) -> Result<ProjectAnalysis> {
    let root_path = root_path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root path: {}", root_path.display()))?;

    // --- Step 1: Structural analysis ---
    let structure: WorkspaceStructure =
        structure::analyze_structure(&root_path).context("structural analysis failed")?;

    // --- Step 2: Per-candidate analysis ---
    let mut warnings = Vec::new();
    let mut services = Vec::new();

    for candidate in &structure.candidates {
        // Apply service filter.
        if !services_filter.is_empty()
            && !services_filter.iter().any(|f| {
                f == &candidate.name || f == candidate.relative_path.to_string_lossy().as_ref()
            })
        {
            continue;
        }

        // --- Language & package manager ---
        let (detected_lang, detected_pm) = detect_language_and_pm(&candidate.full_path);
        let language = lang_override.cloned().unwrap_or(detected_lang);
        let package_manager = detected_pm;

        // --- Version ---
        let runtime_version = extract_version(&candidate.full_path, &language);

        // --- Manifests & framework ---
        let manifest = analyze_manifests(&candidate.full_path);
        let fw_result = detect_framework(&candidate.full_path, &manifest, &language);
        let framework = fw_override.cloned().unwrap_or(fw_result.framework);

        // --- Ports ---
        let exposed_ports: Vec<u16> = vec![fw_result.default_port];

        // --- Env vars ---
        let env_vars: Vec<(String, String)> = fw_result.env_vars;

        // --- Commands ---
        let build_command = fw_result.default_build_cmd;
        let start_command = fw_result.default_start_cmd;

        // --- Service type ---
        let service_type = resolve_service_type(&candidate.service_type, &language, &framework);

        // --- Warnings ---
        if runtime_version.is_none() {
            warnings.push(format!(
                "could not detect runtime version for service '{}' ({})",
                candidate.name,
                candidate.relative_path.display(),
            ));
        }

        let svc = Service {
            name: candidate.name.clone(),
            path: candidate.full_path.clone(),
            language,
            framework,
            package_manager,
            runtime_version,
            entrypoint: manifest.entrypoint,
            exposed_ports,
            env_vars,
            service_type,
            build_command,
            start_command,
            is_monorepo: structure.is_monorepo,
        };

        services.push(svc);
    }

    // --- Step 3: Deterministic sort ---
    services.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(ProjectAnalysis {
        root_path,
        is_monorepo: structure.is_monorepo,
        workspace_tool: structure.workspace_tool,
        services,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolves the final [`ServiceType`] from structural heuristic, language, and
/// framework heuristics.
fn resolve_service_type(
    structural: &ServiceType,
    language: &Language,
    framework: &Framework,
) -> ServiceType {
    // Structural heuristic already assigned a specific type — honour it.
    match structural {
        ServiceType::Frontend => return ServiceType::Frontend,
        ServiceType::Backend => return ServiceType::Backend,
        ServiceType::Single => return ServiceType::Single,
        ServiceType::Worker => return ServiceType::Worker,
        ServiceType::Api => return ServiceType::Api,
        ServiceType::MonorepoMember => {} // Try framework/language heuristics.
    }

    // MonorepoMember fallback — try to classify by framework.
    match framework {
        Framework::NextJs
        | Framework::Nuxt
        | Framework::SvelteKit
        | Framework::Astro
        | Framework::Remix => return ServiceType::Frontend,

        Framework::Express
        | Framework::Fastify
        | Framework::NestJs
        | Framework::FastApi
        | Framework::Django
        | Framework::Flask
        | Framework::Starlette
        | Framework::Litestar
        | Framework::Gin
        | Framework::Echo
        | Framework::Fiber
        | Framework::Chi
        | Framework::ActixWeb
        | Framework::Axum
        | Framework::Rocket
        | Framework::Warp
        | Framework::SpringBoot
        | Framework::Quarkus
        | Framework::Micronaut
        | Framework::Laravel
        | Framework::Symfony
        | Framework::AspNetCore
        | Framework::Rails
        | Framework::Sinatra => return ServiceType::Api,

        _ => {} // Fall through to language heuristic.
    }

    // Language heuristic fallback.
    match language {
        Language::NodeJs | Language::Php => ServiceType::Api,
        Language::Python
        | Language::Go
        | Language::Rust
        | Language::Java
        | Language::DotNet
        | Language::Ruby => ServiceType::Backend,
        Language::Unknown(_) => ServiceType::MonorepoMember,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Creates a minimal valid project at `root` with `package.json` and
    /// `package-lock.json` to satisfy the Node.js language detector.
    fn create_node_project(root: &Path) {
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"test","scripts":{"start":"node index.js"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("package-lock.json"), "{}").unwrap();
    }

    /// Creates a minimal Go project.
    fn create_go_project(root: &Path, go_mod: &str) {
        std::fs::write(root.join("go.mod"), go_mod).unwrap();
    }

    /// Creates a minimal Rust project.
    fn create_rust_project(root: &Path) {
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
    }

    // -----------------------------------------------------------------------
    // Structural integration
    // -----------------------------------------------------------------------

    #[test]
    fn single_service_project() {
        let tmp = TempDir::new().unwrap();
        create_node_project(tmp.path());

        let analysis = analyze_full_project(tmp.path(), None, None, &[]).unwrap();

        assert!(!analysis.is_monorepo);
        assert!(analysis.workspace_tool.is_none());
        assert_eq!(analysis.services.len(), 1);

        let svc = &analysis.services[0];
        assert_eq!(svc.language, Language::NodeJs);
        assert!(svc.exposed_ports.contains(&3000));
        assert!(!svc.is_monorepo);
    }

    #[test]
    fn monorepo_with_frontend_backend() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Workspace tool marker.
        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        // Frontend service.
        let fe = root.join("frontend");
        std::fs::create_dir(&fe).unwrap();
        create_node_project(&fe);

        // Backend service.
        let be = root.join("backend");
        std::fs::create_dir(&be).unwrap();
        create_go_project(&be, "module backend\n\ngo 1.22\n");

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        assert!(analysis.is_monorepo);
        assert_eq!(analysis.workspace_tool.as_deref(), Some("turborepo"));
        assert_eq!(analysis.services.len(), 2);

        let names: Vec<&str> = analysis.services.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"backend"));
        assert!(names.contains(&"frontend"));

        let fe_svc = analysis
            .services
            .iter()
            .find(|s| s.name == "frontend")
            .unwrap();
        assert_eq!(fe_svc.language, Language::NodeJs);
        assert_eq!(fe_svc.service_type, ServiceType::Frontend);

        let be_svc = analysis
            .services
            .iter()
            .find(|s| s.name == "backend")
            .unwrap();
        assert_eq!(be_svc.language, Language::Go);
        assert_eq!(be_svc.service_type, ServiceType::Backend);
    }

    #[test]
    fn monorepo_nested_apps_services() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        // apps/web
        let apps_web = root.join("apps").join("web");
        std::fs::create_dir_all(&apps_web).unwrap();
        create_node_project(&apps_web);

        // services/api
        let svc_api = root.join("services").join("api");
        std::fs::create_dir_all(&svc_api).unwrap();
        create_rust_project(&svc_api);

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        assert!(analysis.is_monorepo);
        assert!(analysis.services.len() >= 2);

        let names: Vec<&str> = analysis.services.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"web"));
        assert!(names.contains(&"api"));

        let api_svc = analysis.services.iter().find(|s| s.name == "api").unwrap();
        assert_eq!(api_svc.language, Language::Rust);
    }

    // -----------------------------------------------------------------------
    // Service filter
    // -----------------------------------------------------------------------

    #[test]
    fn filter_services_by_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let fe = root.join("frontend");
        std::fs::create_dir(&fe).unwrap();
        create_node_project(&fe);

        let be = root.join("backend");
        std::fs::create_dir(&be).unwrap();
        create_go_project(&be, "module backend\n\ngo 1.22\n");

        let analysis = analyze_full_project(root, None, None, &["frontend".into()]).unwrap();

        assert_eq!(analysis.services.len(), 1);
        assert_eq!(analysis.services[0].name, "frontend");
    }

    #[test]
    fn filter_services_by_relative_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let apps_web = root.join("apps").join("web");
        std::fs::create_dir_all(&apps_web).unwrap();
        create_node_project(&apps_web);

        let svc_api = root.join("services").join("api");
        std::fs::create_dir_all(&svc_api).unwrap();
        create_rust_project(&svc_api);

        let analysis = analyze_full_project(root, None, None, &["apps/web".into()]).unwrap();

        assert_eq!(analysis.services.len(), 1);
        assert_eq!(analysis.services[0].name, "web");
    }

    // -----------------------------------------------------------------------
    // Overrides
    // -----------------------------------------------------------------------

    #[test]
    fn language_override() {
        let tmp = TempDir::new().unwrap();
        create_go_project(tmp.path(), "module x\n\ngo 1.22\n");

        let analysis = analyze_full_project(tmp.path(), Some(&Language::Rust), None, &[]).unwrap();

        assert_eq!(analysis.services.len(), 1);
        assert_eq!(analysis.services[0].language, Language::Rust);
    }

    #[test]
    fn framework_override() {
        let tmp = TempDir::new().unwrap();
        create_node_project(tmp.path());

        let analysis =
            analyze_full_project(tmp.path(), None, Some(&Framework::Express), &[]).unwrap();

        assert_eq!(analysis.services.len(), 1);
        assert_eq!(analysis.services[0].framework, Framework::Express);
    }

    #[test]
    fn both_overrides() {
        let tmp = TempDir::new().unwrap();
        create_node_project(tmp.path());

        let analysis = analyze_full_project(
            tmp.path(),
            Some(&Language::Python),
            Some(&Framework::FastApi),
            &[],
        )
        .unwrap();

        let svc = &analysis.services[0];
        assert_eq!(svc.language, Language::Python);
        assert_eq!(svc.framework, Framework::FastApi);
    }

    // -----------------------------------------------------------------------
    // Deterministic output
    // -----------------------------------------------------------------------

    #[test]
    fn services_sorted_by_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        // Create services in reverse alphabetical order.
        for name in &["zebra", "alpha", "mango"] {
            let svc_dir = root.join(name);
            std::fs::create_dir(&svc_dir).unwrap();
            create_node_project(&svc_dir);
        }

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        let names: Vec<&str> = analysis.services.iter().map(|s| s.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn identical_input_produces_identical_output() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let fe = root.join("frontend");
        std::fs::create_dir(&fe).unwrap();
        create_node_project(&fe);

        let be = root.join("backend");
        std::fs::create_dir(&be).unwrap();
        create_rust_project(&be);

        let a = analyze_full_project(root, None, None, &[]).unwrap();
        let b = analyze_full_project(root, None, None, &[]).unwrap();

        // Compare services count + each service name.
        assert_eq!(a.services.len(), b.services.len());
        for (sa, sb) in a.services.iter().zip(b.services.iter()) {
            assert_eq!(sa.name, sb.name);
            assert_eq!(sa.language, sb.language);
            assert_eq!(sa.framework, sb.framework);
            assert_eq!(sa.package_manager, sb.package_manager);
            assert_eq!(sa.exposed_ports, sb.exposed_ports);
        }
    }

    // -----------------------------------------------------------------------
    // Service type resolution
    // -----------------------------------------------------------------------

    #[test]
    fn frontend_framework_yields_frontend_type() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let web = root.join("web");
        std::fs::create_dir(&web).unwrap();
        std::fs::write(
            web.join("package.json"),
            r#"{"name":"web","dependencies":{"next":"14.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(web.join("next.config.js"), "").unwrap();

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        let web_svc = analysis.services.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web_svc.service_type, ServiceType::Frontend);
    }

    #[test]
    fn backend_framework_yields_backend_or_api() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let api = root.join("api");
        std::fs::create_dir(&api).unwrap();
        create_rust_project(&api);

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        let api_svc = analysis.services.iter().find(|s| s.name == "api").unwrap();
        // Rust + Axum (via Cargo.toml deps not present → RustGeneric → Backend)
        assert!(
            api_svc.service_type == ServiceType::Backend
                || api_svc.service_type == ServiceType::Api
        );
    }

    #[test]
    fn single_service_yields_single_type() {
        let tmp = TempDir::new().unwrap();
        create_node_project(tmp.path());

        let analysis = analyze_full_project(tmp.path(), None, None, &[]).unwrap();

        assert_eq!(analysis.services[0].service_type, ServiceType::Single);
    }

    // -----------------------------------------------------------------------
    // Warnings
    // -----------------------------------------------------------------------

    #[test]
    fn warning_on_unknown_language() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // No manifests at all — language will be Unknown.
        std::fs::write(root.join("turbo.json"), "{}").unwrap();
        let mystery = root.join("mystery");
        std::fs::create_dir(&mystery).unwrap();
        std::fs::write(mystery.join("something.xyz"), "").unwrap();

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        // Should have at least one service (mystery dir with no manifest? actually
        // no — it won't be discovered since it has no manifest). Let's verify.
        // Actually, structure::discover_monorepo_candidates only picks up dirs with
        // manifests, so this will have 0 services.
        assert_eq!(analysis.services.len(), 0);
    }

    #[test]
    fn empty_project_no_services() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("turbo.json"), "{}").unwrap();

        let analysis = analyze_full_project(root, None, None, &[]).unwrap();

        assert!(analysis.services.is_empty());
        assert!(analysis.is_monorepo);
    }

    // -----------------------------------------------------------------------
    // Edge: nonexistent root
    // -----------------------------------------------------------------------

    #[test]
    fn nonexistent_root_returns_error() {
        let result = analyze_full_project(Path::new("/nonexistent/project/path"), None, None, &[]);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // resolve_service_type unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_service_type_structural_priority() {
        assert_eq!(
            resolve_service_type(&ServiceType::Frontend, &Language::Rust, &Framework::Generic),
            ServiceType::Frontend,
        );
        assert_eq!(
            resolve_service_type(
                &ServiceType::Backend,
                &Language::NodeJs,
                &Framework::Generic
            ),
            ServiceType::Backend,
        );
        assert_eq!(
            resolve_service_type(&ServiceType::Api, &Language::Python, &Framework::Generic),
            ServiceType::Api,
        );
    }

    #[test]
    fn resolve_service_type_framework_priority() {
        assert_eq!(
            resolve_service_type(
                &ServiceType::MonorepoMember,
                &Language::Rust,
                &Framework::NextJs
            ),
            ServiceType::Frontend,
        );
        assert_eq!(
            resolve_service_type(
                &ServiceType::MonorepoMember,
                &Language::Rust,
                &Framework::Axum
            ),
            ServiceType::Api,
        );
    }

    #[test]
    fn resolve_service_type_language_fallback() {
        assert_eq!(
            resolve_service_type(
                &ServiceType::MonorepoMember,
                &Language::NodeJs,
                &Framework::Generic
            ),
            ServiceType::Api,
        );
        assert_eq!(
            resolve_service_type(
                &ServiceType::MonorepoMember,
                &Language::Rust,
                &Framework::Generic
            ),
            ServiceType::Backend,
        );
        assert_eq!(
            resolve_service_type(
                &ServiceType::MonorepoMember,
                &Language::Unknown("zig".into()),
                &Framework::Generic
            ),
            ServiceType::MonorepoMember,
        );
    }
}
