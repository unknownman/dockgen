use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tera::Tera;

use crate::models::{GeneratedFile, GenerationConfig, Language, ProjectAnalysis};
use crate::templates::resolve_dockerignore_template;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate `.dockerignore` files for the project.
///
/// * **Monorepo** – one `.dockerignore` per service subdirectory.
/// * **Always** – a root-level `.dockerignore` is emitted. For polyglot
///   monorepos, ignore rules from all detected service languages are merged.
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

        // Root-level .dockerignore: merge rules from all service languages.
        let root_content = synthesize_polyglot_dockerignore(analysis, tera)?;
        files.push(GeneratedFile {
            relative_path: ".dockerignore".into(),
            content: root_content,
            description: "root .dockerignore (polyglot merged)".into(),
        });
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// Polyglot root synthesis
// ---------------------------------------------------------------------------

/// Render the ignore template for every unique language in the project,
/// deduplicate lines, and produce a single merged `.dockerignore`.
fn synthesize_polyglot_dockerignore(analysis: &ProjectAnalysis, tera: &Tera) -> Result<String> {
    let mut seen_languages = BTreeSet::new();
    let mut all_lines = Vec::new();

    // Always include the generic ignore patterns.
    let generic_tpl = resolve_dockerignore_template(&Language::Unknown("root".into()));
    if let Ok(content) = tera.render(generic_tpl, &tera::Context::new()) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !all_lines.contains(&trimmed.to_string()) {
                all_lines.push(trimmed.to_string());
            }
        }
    }

    // Collect unique languages.
    for service in &analysis.services {
        if seen_languages.insert(service.language.to_string()) {
            let tpl_path = resolve_dockerignore_template(&service.language);
            if let Ok(content) = tera.render(tpl_path, &tera::Context::new()) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !all_lines.contains(&trimmed.to_string()) {
                        all_lines.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    // Sort for deterministic output.
    all_lines.sort();
    all_lines.dedup();

    Ok(all_lines.join("\n") + "\n")
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
            .any(|f| f.relative_path == std::path::Path::new("frontend/.dockerignore")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == std::path::Path::new("backend/.dockerignore")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == std::path::Path::new(".dockerignore")));
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

    #[test]
    fn polyglot_root_merges_all_languages() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs),
            make_service("api", Language::Python),
            make_service("worker", Language::Go),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerignores(&analysis, &config, &tera).unwrap();
        let root = files
            .iter()
            .find(|f| f.relative_path == std::path::Path::new(".dockerignore"))
            .expect("root .dockerignore missing");

        // Should contain rules from all three languages.
        assert!(root.content.contains("node_modules"), "missing Node rule");
        assert!(root.content.contains("__pycache__"), "missing Python rule");
        assert!(root.content.contains("vendor"), "missing Go rule");
        assert!(root.description.contains("polyglot"));
    }

    #[test]
    fn polyglot_root_is_deterministic() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs),
            make_service("api", Language::Python),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();
        let tera = tera_engine();

        let f1 = generate_dockerignores(&analysis, &config, &tera).unwrap();
        let f2 = generate_dockerignores(&analysis, &config, &tera).unwrap();
        let root1 = f1
            .iter()
            .find(|f| f.relative_path == std::path::Path::new(".dockerignore"))
            .unwrap();
        let root2 = f2
            .iter()
            .find(|f| f.relative_path == std::path::Path::new(".dockerignore"))
            .unwrap();
        assert_eq!(root1.content, root2.content);
    }
}
