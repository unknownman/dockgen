use anyhow::{Context, Result};
use tera::Tera;

use crate::models::{
    Framework, GeneratedFile, GenerationConfig, InfraKind, InfraService, ProjectAnalysis,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a `docker-compose.yml` if `config.emit_compose` is `true`.
///
/// Application services and infrastructure services (Postgres, Redis, etc.)
/// are both rendered. Infrastructure services are filtered by
/// `InteractiveAnswers.include_infra_in_compose` when present, otherwise
/// by their default `is_attached_to_compose` flag.
pub fn generate_docker_compose(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
    tera: &Tera,
) -> Result<Option<GeneratedFile>> {
    if !config.emit_compose {
        return Ok(None);
    }

    if analysis.services.is_empty() && analysis.detected_infrastructures.is_empty() {
        anyhow::bail!("no services to generate docker-compose.yml for");
    }

    let is_single_service = !analysis.is_monorepo || analysis.services.len() == 1;
    let force_single = config.force_single && analysis.services.len() > 1;

    // --- Infrastructure services ---
    let (mut infra_entries, volume_names) = build_infra_entries(analysis, config);
    infra_entries.sort_by(|a, b| {
        let na = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let nb = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        na.cmp(nb)
    });

    // Resolve which infra services are included (for depends_on computation).
    let included_infra_refs = resolve_compose_infra(analysis, config);

    // --- Application service entries ---
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
                    s.replace('\\', "/")
                }
            })
            .unwrap_or_else(|_| ".".into());

        let dockerfile_path = if is_single_service {
            "Dockerfile".into()
        } else if force_single {
            format!("Dockerfile.{}", service.name)
        } else {
            "Dockerfile".into()
        };

        let port = custom_port_for_service(&service.name, config)
            .or_else(|| config.port_overrides.get(idx).copied())
            .or_else(|| service.exposed_ports.first().copied())
            .unwrap_or(8080);

        let env: Vec<serde_json::Value> = service
            .env_vars
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();

        // Build depends_on from infrastructure connections.
        let depends_on = build_depends_on(service, &included_infra_refs);

        // Prisma migration command override.
        let command = if let Some(ref answers) = config.interactive_answers {
            let run_prisma = answers.run_prisma_migrations.unwrap_or(false);
            if run_prisma
                && service
                    .build_command
                    .as_deref()
                    .is_some_and(|c| c.contains("prisma"))
            {
                Some(format!(
                    "npx prisma migrate deploy && {}",
                    service.start_command.as_deref().unwrap_or("npm start")
                ))
            } else {
                None
            }
        } else {
            None
        };

        let mut entry = serde_json::json!({
            "name": slug,
            "relative_path": relative_path,
            "dockerfile_path": dockerfile_path,
            "ports": vec![port],
            "environment": env,
        });

        if !depends_on.is_empty() {
            entry["depends_on"] = serde_json::json!(depends_on);
        }
        if let Some(cmd) = command {
            entry["command"] = serde_json::json!(cmd);
        }

        svc_entries.push(entry);
    }

    // --- Worker service entries ---
    let worker_entries = build_worker_entries(analysis, config);

    // --- Build Tera context ---
    let mut ctx = tera::Context::new();
    ctx.insert("services", &svc_entries);

    if !worker_entries.is_empty() {
        ctx.insert("worker_services", &worker_entries);
    }

    if !infra_entries.is_empty() {
        ctx.insert("infra_services", &infra_entries);
    }
    if !volume_names.is_empty() {
        ctx.insert("volumes", &volume_names);
    }

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
// Infrastructure helpers
// ---------------------------------------------------------------------------

/// Build Tera-compatible JSON entries and volume name list for infrastructure
/// services.
fn build_infra_entries(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
) -> (Vec<serde_json::Value>, Vec<String>) {
    let included = resolve_compose_infra(analysis, config);
    let mut entries = Vec::new();
    let mut volumes: Vec<String> = Vec::new();

    for infra in &included {
        let port = config
            .interactive_answers
            .as_ref()
            .and_then(|a| a.custom_service_ports.get(&infra.name))
            .copied()
            .unwrap_or(infra.port);

        let ports = if port > 0 { vec![port] } else { vec![] };

        let env: Vec<serde_json::Value> = infra
            .env_vars
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();

        let vol_name = format!("{}data", infra.name);
        let vol_mount = format!("{vol_name}:{}", infra_volume_mount_path(infra.kind));
        let persists = infra_persists_data(infra.kind);

        // Only include volume mounts for infrastructure kinds that persist data.
        let vol_array = if persists { vec![vol_mount] } else { vec![] };

        entries.push(serde_json::json!({
            "name": infra.name,
            "image": infra.image,
            "ports": ports,
            "environment": env,
            "volumes": vol_array,
        }));

        if persists {
            volumes.push(vol_name);
        }
    }

    volumes.sort();
    volumes.dedup();

    (entries, volumes)
}

/// Resolve which infrastructure services should appear in compose, using
/// interactive answers when available, falling back to detection defaults.
fn resolve_compose_infra<'a>(
    analysis: &'a ProjectAnalysis,
    config: &GenerationConfig,
) -> Vec<&'a InfraService> {
    if let Some(ref answers) = config.interactive_answers {
        analysis
            .detected_infrastructures
            .iter()
            .filter(|i| answers.include_infra_in_compose.contains(&i.kind))
            .collect()
    } else {
        analysis
            .detected_infrastructures
            .iter()
            .filter(|i| i.is_attached_to_compose)
            .collect()
    }
}

/// Retrieve a custom port for a named service from interactive answers.
fn custom_port_for_service(service_name: &str, config: &GenerationConfig) -> Option<u16> {
    config
        .interactive_answers
        .as_ref()
        .and_then(|a| a.custom_service_ports.get(service_name))
        .copied()
}

/// Build `depends_on` list for an application service based on detected
/// infrastructure that it uses.
fn build_depends_on(service: &crate::models::Service, infra: &[&InfraService]) -> Vec<String> {
    let mut deps = Vec::new();
    for detected in infra {
        if service_uses_infra(service, detected.kind) {
            deps.push(detected.name.clone());
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Returns `true` if the service appears to connect to the given infrastructure
/// kind, based on env vars, build/start commands, and package name.
fn service_uses_infra(service: &crate::models::Service, kind: InfraKind) -> bool {
    // Check framework-level env vars.
    let env_match = service.env_vars.iter().any(|(k, _)| {
        let upper = k.to_ascii_uppercase();
        match kind {
            InfraKind::Postgres => upper.contains("DATABASE_URL") || upper.contains("POSTGRES"),
            InfraKind::Mysql => upper.contains("DATABASE_URL") || upper.contains("MYSQL"),
            InfraKind::Redis => upper.contains("REDIS"),
            InfraKind::Mongo => upper.contains("MONGO") || upper.contains("MONGODB"),
            InfraKind::RabbitMq => upper.contains("AMQP") || upper.contains("RABBITMQ"),
            InfraKind::Kafka => upper.contains("KAFKA"),
            InfraKind::Sqlite => false,
        }
    });
    if env_match {
        return true;
    }

    // Check build/start commands for infra hints.
    let cmd_text = service
        .build_command
        .iter()
        .chain(service.start_command.iter())
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // Check package_name for Prisma.
    let pkg_text = service.package_name.as_deref().unwrap_or("");

    match kind {
        InfraKind::Postgres => {
            cmd_text.contains("prisma")
                || pkg_text.contains("prisma")
                || pkg_text.contains("@prisma/client")
        }
        InfraKind::Mysql => {
            cmd_text.contains("prisma") && cmd_text.contains("mysql") || pkg_text.contains("prisma")
        }
        InfraKind::Redis => cmd_text.contains("redis") || pkg_text.contains("redis"),
        InfraKind::Mongo => {
            cmd_text.contains("prisma") && cmd_text.contains("mongodb")
                || pkg_text.contains("prisma")
        }
        InfraKind::RabbitMq => cmd_text.contains("rabbitmq") || pkg_text.contains("rabbitmq"),
        InfraKind::Kafka => cmd_text.contains("kafka") || pkg_text.contains("kafka"),
        InfraKind::Sqlite => false,
    }
}

/// Returns `true` if the given service is detected as Laravel.
fn is_laravel_service(service: &crate::models::Service) -> bool {
    service.language == crate::models::Language::Php && service.framework == Framework::Laravel
}

/// Build worker service entries for Laravel queue workers.
fn build_worker_entries(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
) -> Vec<serde_json::Value> {
    let create_worker = config
        .interactive_answers
        .as_ref()
        .and_then(|a| a.create_queue_worker)
        .unwrap_or(false);
    if !create_worker {
        return Vec::new();
    }

    let is_single_service = !analysis.is_monorepo || analysis.services.len() == 1;

    analysis
        .services
        .iter()
        .filter(|svc| is_laravel_service(svc))
        .map(|svc| {
            let slug = slugify(&svc.name);
            let worker_slug = format!("{slug}-worker");

            let relative_path = svc
                .path
                .strip_prefix(&analysis.root_path)
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    if s.is_empty() {
                        ".".into()
                    } else {
                        s.replace('\\', "/")
                    }
                })
                .unwrap_or_else(|_| ".".into());

            let dockerfile_path = if is_single_service {
                "Dockerfile".into()
            } else if config.force_single {
                format!("Dockerfile.{}", svc.name)
            } else {
                "Dockerfile".into()
            };

            serde_json::json!({
                "name": worker_slug,
                "relative_path": relative_path,
                "dockerfile_path": dockerfile_path,
                "ports": [],
                "environment": [],
                "command": "php artisan queue:work",
                "depends_on": [slug],
            })
        })
        .collect()
}

/// Whether an infrastructure kind persists data to a named volume.
fn infra_persists_data(kind: InfraKind) -> bool {
    matches!(
        kind,
        InfraKind::Postgres
            | InfraKind::Mysql
            | InfraKind::Redis
            | InfraKind::Mongo
            | InfraKind::Kafka
            | InfraKind::RabbitMq
    )
}

/// The in-container mount path for infrastructure data volumes.
fn infra_volume_mount_path(kind: InfraKind) -> &'static str {
    match kind {
        InfraKind::Postgres => "/var/lib/postgresql/data",
        InfraKind::Mysql => "/var/lib/mysql",
        InfraKind::Redis => "/data",
        InfraKind::Mongo => "/data/db",
        InfraKind::Kafka => "/var/lib/kafka/data",
        InfraKind::RabbitMq => "/var/lib/rabbitmq",
        InfraKind::Sqlite => "/data",
    }
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
    use std::collections::BTreeMap;
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

    fn make_analysis(services: Vec<Service>, infra: Vec<InfraService>) -> ProjectAnalysis {
        ProjectAnalysis {
            root_path: PathBuf::from("/project"),
            is_monorepo: true,
            workspace_tool: Some("turborepo".into()),
            services,
            detected_infrastructures: infra,
            warnings: vec![],
        }
    }

    fn tera_engine() -> Tera {
        create_tera_engine().expect("tera engine init failed")
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

    fn make_infra(kind: InfraKind, name: &str) -> InfraService {
        InfraService {
            kind,
            name: name.into(),
            image: kind.default_image().into(),
            port: kind.default_port(),
            env_vars: kind
                .default_env()
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            is_attached_to_compose: kind != InfraKind::Sqlite,
            source: InfraSource::EnvVar("DETECTED".into()),
        }
    }

    // -----------------------------------------------------------------------
    // Existing tests (backward-compatible)
    // -----------------------------------------------------------------------

    #[test]
    fn compose_not_generated_when_disabled() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let analysis = make_analysis(svcs, vec![]);
        let config = GenerationConfig {
            emit_compose: false,
            ..default_config()
        };
        let tera = tera_engine();

        let result = generate_docker_compose(&analysis, &config, &tera).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn compose_generated_when_enabled() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let analysis = make_analysis(svcs, vec![]);
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
        let analysis = make_analysis(svcs, vec![]);
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
        let analysis = make_analysis(svcs, vec![]);
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
        let analysis = make_analysis(vec![], vec![]);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        assert!(generate_docker_compose(&analysis, &config, &tera).is_err());
    }

    // -----------------------------------------------------------------------
    // Infrastructure integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn compose_with_infra_services() {
        let svcs = vec![make_service("api", Language::NodeJs, Framework::Express)];
        let infra = vec![make_infra(InfraKind::Postgres, "postgres")];
        let analysis = make_analysis(svcs, infra);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        assert!(file.content.contains("api:"));
        assert!(file.content.contains("postgres:"));
        assert!(file.content.contains("postgres:16-alpine"));
        assert!(file.content.contains("5432:5432"));
        assert!(file.content.contains("POSTGRES_USER=app"));
        assert!(file.content.contains("volumes:"));
    }

    #[test]
    fn compose_infra_sorted_by_kind() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let infra = vec![
            make_infra(InfraKind::Redis, "redis"),
            make_infra(InfraKind::Postgres, "postgres"),
        ];
        let analysis = make_analysis(svcs, infra);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        // Infra services appear after application services.
        let api_pos = file.content.find("api:").unwrap();
        let pg_pos = file.content.find("postgres:").unwrap();
        let redis_pos = file.content.find("redis:").unwrap();
        assert!(api_pos < pg_pos);
        assert!(pg_pos < redis_pos);
    }

    #[test]
    fn compose_infra_with_interactive_answers() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let infra = vec![
            make_infra(InfraKind::Postgres, "postgres"),
            make_infra(InfraKind::Redis, "redis"),
        ];
        let analysis = make_analysis(svcs, infra);
        let mut config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        config.interactive_answers = Some(InteractiveAnswers {
            include_infra_in_compose: vec![InfraKind::Postgres],
            ..Default::default()
        });
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        assert!(file.content.contains("postgres:"));
        // Redis should NOT be included — not in answers.
        assert!(!file.content.contains("redis:"));
    }

    #[test]
    fn compose_infra_custom_port_from_answers() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let infra = vec![make_infra(InfraKind::Postgres, "postgres")];
        let analysis = make_analysis(svcs, infra);

        let mut custom_ports = BTreeMap::new();
        custom_ports.insert("postgres".into(), 5433);

        let mut config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        config.interactive_answers = Some(InteractiveAnswers {
            include_infra_in_compose: vec![InfraKind::Postgres],
            custom_service_ports: custom_ports,
            ..Default::default()
        });
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        assert!(file.content.contains("5433:5433"));
    }

    #[test]
    fn compose_infra_volumes_block() {
        let svcs = vec![make_service("api", Language::NodeJs, Framework::Express)];
        let infra = vec![
            make_infra(InfraKind::Postgres, "postgres"),
            make_infra(InfraKind::Redis, "redis"),
        ];
        let analysis = make_analysis(svcs, infra);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        // Top-level volumes block should list the named volumes.
        assert!(file.content.contains("volumes:"));
        assert!(file.content.contains("postgresdata:"));
        assert!(file.content.contains("redisdata:"));
    }

    #[test]
    fn compose_rabbitmq_volume_persistence() {
        let svcs = vec![make_service("api", Language::NodeJs, Framework::Express)];
        let infra = vec![make_infra(InfraKind::RabbitMq, "rabbitmq")];
        let analysis = make_analysis(svcs, infra);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        // Service block should mount the named volume.
        assert!(file.content.contains("rabbitmqdata:/var/lib/rabbitmq"));
        // Top-level volumes block should declare the named volume.
        assert!(file.content.contains("rabbitmqdata:"));
    }

    #[test]
    fn compose_no_infra_services_block() {
        let svcs = vec![make_service("api", Language::Go, Framework::Gin)];
        let analysis = make_analysis(svcs, vec![]);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };
        let tera = tera_engine();

        let file = generate_docker_compose(&analysis, &config, &tera)
            .unwrap()
            .unwrap();

        // No "Infrastructure Services" comment.
        assert!(!file.content.contains("Infrastructure Services"));
    }

    #[test]
    fn custom_port_for_service_helper() {
        let mut answers = InteractiveAnswers::default();
        answers.custom_service_ports.insert("api".into(), 4000);

        let config = GenerationConfig {
            interactive_answers: Some(answers),
            ..default_config()
        };

        assert_eq!(custom_port_for_service("api", &config), Some(4000));
        assert_eq!(custom_port_for_service("web", &config), None);
    }

    #[test]
    fn custom_port_for_service_no_answers() {
        let config = default_config();
        assert_eq!(custom_port_for_service("api", &config), None);
    }
}
