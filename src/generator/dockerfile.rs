use std::collections::HashMap;

use anyhow::{Context, Result};
use tera::Tera;

use crate::models::{GeneratedFile, GenerationConfig, ProjectAnalysis, Service};
use crate::templates::{create_tera_engine, resolve_dockerfile_template};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate `Dockerfile` contents for every service in the project.
///
/// * **Single-service / `force_single`** – a single `Dockerfile` is placed at
///   the output root.
/// * **Monorepo** – a separate `Dockerfile` is placed inside each service's
///   relative directory.
pub fn generate_dockerfiles(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
    tera: &Tera,
) -> Result<Vec<GeneratedFile>> {
    if analysis.services.is_empty() {
        anyhow::bail!("no services to generate Dockerfiles for");
    }

    let single = config.force_single || !analysis.is_monorepo || analysis.services.len() == 1;

    let mut files = Vec::new();

    for (idx, service) in analysis.services.iter().enumerate() {
        let ctx = build_dockerfile_context(service, config, idx);
        let tpl_path = resolve_dockerfile_template(&service.language, &service.framework);

        let content = tera.render(tpl_path, &ctx).with_context(|| {
            format!("failed to render Dockerfile for service '{}'", service.name)
        })?;

        let relative_path = if single {
            "Dockerfile".into()
        } else {
            std::path::PathBuf::from(service.name.clone()).join("Dockerfile")
        };

        files.push(GeneratedFile {
            relative_path,
            content,
            description: format!("Dockerfile for {} ({})", service.name, service.framework),
        });
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// Context builder
// ---------------------------------------------------------------------------

fn build_dockerfile_context(
    service: &Service,
    config: &GenerationConfig,
    service_idx: usize,
) -> tera::Context {
    let mut ctx = tera::Context::new();

    // Port resolution: override by index → detected → default 8080.
    let port = config
        .port_overrides
        .get(service_idx)
        .copied()
        .or_else(|| service.exposed_ports.first().copied())
        .unwrap_or(8080);
    ctx.insert("port", &port);

    // Runtime version with sensible defaults per language.
    let default_version = default_runtime_version(&service.language);
    let version = service
        .runtime_version
        .as_deref()
        .unwrap_or(&default_version);
    ctx.insert("runtime_version", version);

    // Base image variant.
    if let Some(variant) = config.base_image_override {
        ctx.insert("base_image_variant", &variant.to_string());
    }

    // Build & start commands.
    if let Some(ref cmd) = service.build_command {
        ctx.insert("build_command", cmd);
    }
    if let Some(ref cmd) = service.start_command {
        ctx.insert("start_command", cmd);
    }

    // String metadata for template conditionals.
    ctx.insert("package_manager", &service.package_manager.to_string());
    ctx.insert("language", &service.language.to_string());
    ctx.insert("framework", &service.framework.to_string());

    // Environment variables.
    let env_map: Vec<HashMap<&str, &str>> = service
        .env_vars
        .iter()
        .map(|(k, v)| {
            let mut m = HashMap::new();
            m.insert("key", k.as_str());
            m.insert("value", v.as_str());
            m
        })
        .collect();
    ctx.insert("env_vars", &env_map);

    ctx
}

/// Sensible default runtime version strings per language family.
fn default_runtime_version(lang: &crate::models::Language) -> String {
    match lang {
        crate::models::Language::NodeJs => "20".into(),
        crate::models::Language::Python => "3.11".into(),
        crate::models::Language::Go => "1.22".into(),
        crate::models::Language::Rust => "1.78".into(),
        crate::models::Language::Java => "21".into(),
        crate::models::Language::Php => "8.2".into(),
        crate::models::Language::DotNet => "8.0".into(),
        crate::models::Language::Ruby => "3.2".into(),
        crate::models::Language::Unknown(_) => "latest".into(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::path::PathBuf;

    fn make_service(name: &str, lang: Language, fw: Framework) -> Service {
        Service {
            name: name.into(),
            path: PathBuf::from(format!("/project/{name}")),
            language: lang,
            framework: fw,
            package_manager: PackageManager::Unknown,
            runtime_version: None,
            entrypoint: None,
            exposed_ports: vec![],
            env_vars: vec![],
            service_type: ServiceType::Single,
            build_command: None,
            start_command: None,
            is_monorepo: false,
        }
    }

    fn make_analysis(services: Vec<Service>, is_monorepo: bool) -> ProjectAnalysis {
        ProjectAnalysis {
            root_path: PathBuf::from("/project"),
            is_monorepo,
            workspace_tool: None,
            services,
            warnings: vec![],
        }
    }

    fn default_config() -> GenerationConfig {
        GenerationConfig {
            base_image_override: None,
            port_overrides: vec![],
            force_single: false,
            dry_run: false,
            emit_compose: false,
            output_dir: None,
        }
    }

    fn tera_engine() -> Tera {
        create_tera_engine().expect("tera engine init failed")
    }

    #[test]
    fn single_service_generates_root_dockerfile() {
        let svc = make_service("api", Language::Go, Framework::GoGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from("Dockerfile"));
        assert!(files[0].content.contains("golang:"));
    }

    #[test]
    fn monorepo_generates_per_service_dockerfiles() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs, Framework::NextJs),
            make_service("backend", Language::Go, Framework::Gin),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 2);
        // Services are in insertion order: frontend first, then backend.
        assert_eq!(files[0].relative_path, PathBuf::from("frontend/Dockerfile"));
        assert_eq!(files[1].relative_path, PathBuf::from("backend/Dockerfile"));
    }

    #[test]
    fn force_single_overrides_monorepo() {
        let svcs = vec![
            make_service("a", Language::Python, Framework::FastApi),
            make_service("b", Language::Rust, Framework::Axum),
        ];
        let analysis = make_analysis(svcs, true);
        let config = GenerationConfig {
            force_single: true,
            ..default_config()
        };
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .all(|f| f.relative_path == PathBuf::from("Dockerfile")));
    }

    #[test]
    fn port_override_by_index() {
        let svcs = vec![
            make_service("a", Language::NodeJs, Framework::NodeGeneric),
            make_service("b", Language::NodeJs, Framework::NodeGeneric),
        ];
        let analysis = make_analysis(svcs, true);
        let config = GenerationConfig {
            port_overrides: vec![4000, 5000],
            ..default_config()
        };
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert!(files[0].content.contains("EXPOSE 4000"));
        assert!(files[1].content.contains("EXPOSE 5000"));
    }

    #[test]
    fn port_fallback_to_default() {
        let svc = make_service("web", Language::NodeJs, Framework::NodeGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        // No port overrides or detected ports → falls back to 8080.
        assert!(files[0].content.contains("EXPOSE 8080"));
    }

    #[test]
    fn empty_services_is_error() {
        let analysis = make_analysis(vec![], false);
        let config = default_config();
        let tera = tera_engine();

        assert!(generate_dockerfiles(&analysis, &config, &tera).is_err());
    }

    #[test]
    fn runtime_version_defaults_applied() {
        let svc = make_service("api", Language::Rust, Framework::RustGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert!(files[0].content.contains("rust:1.78"));
    }

    #[test]
    fn deterministic_output_for_identical_input() {
        let svc = make_service("api", Language::Go, Framework::GoGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let f1 = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        let f2 = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert_eq!(f1[0].content, f2[0].content);
    }
}
