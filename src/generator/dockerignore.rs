use anyhow::{Context, Result};
use std::path::PathBuf;
use tera::Tera;

use crate::models::{GeneratedFile, GenerationConfig, ProjectAnalysis};
use crate::templates::resolve_dockerignore_template;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate `.dockerignore` files for the project.
///
/// * **Monorepo** – one `.dockerignore` per service subdirectory.
/// * **Always** – a root-level `.dockerignore` is emitted (unless the root
///   service already covers it).
pub fn generate_dockerignores(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
    tera: &Tera,
) -> Result<Vec<GeneratedFile>> {
    if analysis.services.is_empty() {
        anyhow::bail!("no services to generate .dockerignore files for");
    }

    let single = config.force_single || !analysis.is_monorepo || analysis.services.len() == 1;
    let mut files = Vec::new();

    if single {
        // Single-service: root .dockerignore.
        let service = &analysis.services[0];
        let tpl_path = resolve_dockerignore_template(&service.language);
        let content = tera
            .render(tpl_path, &tera::Context::new())
            .with_context(|| {
                format!(
                    "failed to render .dockerignore for service '{}'",
                    service.name
                )
            })?;
        files.push(GeneratedFile {
            relative_path: ".dockerignore".into(),
            content,
            description: format!(".dockerignore for {} ({})", service.name, service.language),
        });
    } else {
        // Monorepo: per-service .dockerignore.
        for service in &analysis.services {
            let tpl_path = resolve_dockerignore_template(&service.language);
            let content = tera
                .render(tpl_path, &tera::Context::new())
                .with_context(|| {
                    format!(
                        "failed to render .dockerignore for service '{}'",
                        service.name
                    )
                })?;
            files.push(GeneratedFile {
                relative_path: PathBuf::from(service.name.clone()).join(".dockerignore"),
                content,
                description: format!(".dockerignore for {} ({})", service.name, service.language),
            });
        }

        // Root-level .dockerignore covering the whole project.
        // Use the first service's language as a sensible default, or the most
        // common one.
        let root_lang = &analysis.services[0].language;
        let tpl_path = resolve_dockerignore_template(root_lang);
        let content = tera
            .render(tpl_path, &tera::Context::new())
            .context("failed to render root .dockerignore")?;
        files.push(GeneratedFile {
            relative_path: ".dockerignore".into(),
            content,
            description: "root .dockerignore".into(),
        });
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::templates::create_tera_engine;

    fn make_service(name: &str, lang: Language) -> Service {
        Service {
            name: name.into(),
            path: PathBuf::from(format!("/project/{name}")),
            language: lang,
            framework: Framework::Generic,
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
    fn single_service_generates_root_dockerignore() {
        let svc = make_service("api", Language::NodeJs);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerignores(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from(".dockerignore"));
        assert!(files[0].content.contains("node_modules"));
    }

    #[test]
    fn monorepo_generates_per_service_and_root() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs),
            make_service("backend", Language::Go),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerignores(&analysis, &config, &tera).unwrap();
        // 2 per-service + 1 root = 3
        assert_eq!(files.len(), 3);
        assert!(files
            .iter()
            .any(|f| f.relative_path == PathBuf::from("frontend/.dockerignore")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == PathBuf::from("backend/.dockerignore")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == PathBuf::from(".dockerignore")));
    }

    #[test]
    fn force_single_prevents_per_service() {
        let svcs = vec![
            make_service("a", Language::Python),
            make_service("b", Language::Rust),
        ];
        let analysis = make_analysis(svcs, true);
        let config = GenerationConfig {
            force_single: true,
            ..default_config()
        };
        let tera = tera_engine();

        let files = generate_dockerignores(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, PathBuf::from(".dockerignore"));
    }

    #[test]
    fn empty_services_is_error() {
        let analysis = make_analysis(vec![], false);
        let config = default_config();
        let tera = tera_engine();

        assert!(generate_dockerignores(&analysis, &config, &tera).is_err());
    }

    #[test]
    fn deterministic_output() {
        let svc = make_service("api", Language::Python);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let f1 = generate_dockerignores(&analysis, &config, &tera).unwrap();
        let f2 = generate_dockerignores(&analysis, &config, &tera).unwrap();
        assert_eq!(f1[0].content, f2[0].content);
    }
}
