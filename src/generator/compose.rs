use anyhow::{Context, Result};
use tera::Tera;

use crate::models::{GeneratedFile, GenerationConfig, ProjectAnalysis};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a `docker-compose.yml` if `config.emit_compose` is `true`.
pub fn generate_docker_compose(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
    tera: &Tera,
) -> Result<Option<GeneratedFile>> {
    if !config.emit_compose {
        return Ok(None);
    }

    if analysis.services.is_empty() {
        anyhow::bail!("no services to generate docker-compose.yml for");
    }

    let is_single_service = !analysis.is_monorepo || analysis.services.len() == 1;
    let force_single = config.force_single && analysis.services.len() > 1;

    let mut svc_entries = Vec::new();

    for (idx, service) in analysis.services.iter().enumerate() {
        let slug = slugify(&service.name);

        let relative_path = service
            .path
            .strip_prefix(&analysis.root_path)
            .map(|p| {
                let s = p.to_string_lossy().to_string();
                if s.is_empty() {
                    ".".into()
                } else {
                    // Normalise to forward slashes for Docker Compose
                    // compatibility on all platforms.
                    s.replace('\\', "/")
                }
            })
            .unwrap_or_else(|_| ".".into());

        // Compute the dockerfile path so docker-compose can locate it.
        let dockerfile_path = if is_single_service {
            "Dockerfile".into()
        } else if force_single {
            format!("Dockerfile.{}", service.name)
        } else {
            "Dockerfile".into()
        };

        let port = config
            .port_overrides
            .get(idx)
            .copied()
            .or_else(|| service.exposed_ports.first().copied())
            .unwrap_or(8080);

        let env: Vec<serde_json::Value> = service
            .env_vars
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();

        svc_entries.push(serde_json::json!({
            "name": slug,
            "relative_path": relative_path,
            "dockerfile_path": dockerfile_path,
            "ports": vec![port],
            "environment": env,
        }));
    }

    let mut ctx = tera::Context::new();
    ctx.insert("services", &svc_entries);

    let content = tera
        .render("compose/docker-compose.yml.tera", &ctx)
        .context("failed to render docker-compose.yml")?;

    Ok(Some(GeneratedFile {
        relative_path: "docker-compose.yml".into(),
        content,
        description: "Docker Compose configuration".into(),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a service name to a URL/identifier-safe slug.
fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::templates::create_tera_engine;
    use std::path::PathBuf;

    fn make_service(name: &str, lang: Language, fw: Framework) -> Service {
        Service {
            name: name.into(),
            path: PathBuf::from(format!("/project/{name}")),
            package_name: None,
            language: lang,
            framework: fw,
            package_manager: PackageManager::Unknown,
            runtime_version: None,
            entrypoint: None,
            exposed_ports: vec![8080],
            env_vars: vec![("RUST_LOG".into(), "info".into())],
            service_type: ServiceType::Api,
            build_command: None,
            start_command: None,
            is_monorepo: true,
        }
    }

    fn make_analysis(services: Vec<Service>) -> ProjectAnalysis {
        ProjectAnalysis {
            root_path: PathBuf::from("/project"),
            is_monorepo: true,
            workspace_tool: Some("turborepo".into()),
            services,
            detected_infrastructures: vec![],
            warnings: vec![],
        }
    }

    fn tera_engine() -> Tera {
        create_tera_engine().expect("tera engine init failed")
    }

    #[test]
    fn compose_not_generated_when_disabled() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let analysis = make_analysis(svcs);
        let config = GenerationConfig {
            emit_compose: false,
            ..default_config()
        };
        let tera = tera_engine();

        let result = generate_docker_compose(&analysis, &config, &tera).unwrap();
        assert!(result.is_none());
    }

    fn default_config() -> GenerationConfig {
        GenerationConfig {
            base_image_override: None,
            port_overrides: vec![],
            force_single: false,
            dry_run: false,
            emit_compose: false,
            output_dir: None,
            interactive: false,
            assume_yes: false,
            interactive_answers: None,
        }
    }

    #[test]
    fn compose_generated_when_enabled() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let analysis = make_analysis(svcs);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let result = generate_docker_compose(&analysis, &config, &tera).unwrap();
        let file = result.expect("expected Some");
        assert_eq!(file.relative_path, PathBuf::from("docker-compose.yml"));
        assert!(file.content.contains("api:"));
        assert!(file.content.contains("8080:8080"));
        assert!(file.content.contains("restart: unless-stopped"));
    }

    #[test]
    fn compose_multi_service() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs, Framework::NextJs),
            make_service("backend", Language::Go, Framework::Gin),
        ];
        let analysis = make_analysis(svcs);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();
        assert!(file.content.contains("frontend:"));
        assert!(file.content.contains("backend:"));
    }

    #[test]
    fn compose_port_overrides() {
        let svcs = vec![
            make_service("a", Language::NodeJs, Framework::NodeGeneric),
            make_service("b", Language::Go, Framework::GoGeneric),
        ];
        let analysis = make_analysis(svcs);
        let config = GenerationConfig {
            emit_compose: true,
            port_overrides: vec![4000, 9090],
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();
        assert!(file.content.contains("4000:4000"));
        assert!(file.content.contains("9090:9090"));
    }

    #[test]
    fn slugify_conversions() {
        assert_eq!(slugify("my-app"), "my-app");
        assert_eq!(slugify("My_App.Service"), "my-app-service");
        assert_eq!(slugify("UPPER"), "upper");
        assert_eq!(slugify("--dashes--"), "dashes");
        assert_eq!(slugify("simple"), "simple");
    }

    #[test]
    fn empty_services_is_error() {
        let analysis = make_analysis(vec![]);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        assert!(generate_docker_compose(&analysis, &config, &tera).is_err());
    }
}
