use crate::models::{Framework, InfraKind, ProjectAnalysis};

// ---------------------------------------------------------------------------
// Question types
// ---------------------------------------------------------------------------

/// A question posed to the user during the interactive wizard.
#[derive(Debug, Clone)]
pub enum InteractiveQuestion {
    /// Whether to attach a detected infrastructure service to docker-compose.yml.
    AttachInfra { kind: InfraKind, default: bool },
    /// Whether to run Prisma migrations automatically on startup.
    RunPrismaMigrations { default: bool },
    /// Whether to generate a Laravel queue worker service.
    LaravelQueueWorker { default: bool },
    /// Confirm or customise a non-standard service port.
    ConfirmServicePort {
        service_name: String,
        default_port: u16,
    },
}

// ---------------------------------------------------------------------------
// Question builder
// ---------------------------------------------------------------------------

/// Build the list of interactive questions based on Phase 1 analysis results.
///
/// Questions are only generated when their trigger condition is met:
/// - `AttachInfra`: every detected infrastructure with `is_attached_to_compose`.
/// - `RunPrismaMigrations`: Prisma schema present in any service.
/// - `LaravelQueueWorker`: Laravel framework detected.
/// - `ConfirmServicePort`: any service whose default port is not in the
///   standard set `{80, 443, 3000, 8080}`.
pub fn build_questions(analysis: &ProjectAnalysis) -> Vec<InteractiveQuestion> {
    let mut questions = Vec::new();

    // --- Infrastructure attach questions ---
    for infra in &analysis.detected_infrastructures {
        if infra.is_attached_to_compose {
            questions.push(InteractiveQuestion::AttachInfra {
                kind: infra.kind,
                default: true,
            });
        }
    }

    // --- Framework-specific questions ---
    let has_prisma = analysis.services.iter().any(|svc| {
        // Prisma is Node.js or general — check if any manifest references
        // prisma in scripts or dependencies.
        let is_node = matches!(
            svc.framework,
            Framework::NextJs | Framework::NestJs | Framework::NodeGeneric
        );
        let cmd_refers_to_prisma = svc
            .build_command
            .as_deref()
            .is_some_and(|cmd| cmd.contains("prisma"));
        let pkg_name_is_prisma = svc.package_name.as_deref().is_some_and(|p| {
            p.contains("prisma") || p.contains("@prisma/client")
        });
        (is_node && cmd_refers_to_prisma) || pkg_name_is_prisma
    });

    // Also check for Prisma schema presence via the infra source.
    let prisma_detected = analysis.detected_infrastructures.iter().any(|infra| {
        matches!(infra.source, crate::models::InfraSource::PrismaSchema)
            || matches!(
                infra.source,
                crate::models::InfraSource::ConfigFile(ref p) if p.contains("prisma")
            )
    });

    if has_prisma || prisma_detected {
        questions.push(InteractiveQuestion::RunPrismaMigrations { default: true });
    }

    let has_laravel = analysis
        .services
        .iter()
        .any(|svc| matches!(svc.framework, Framework::Laravel));

    if has_laravel {
        questions.push(InteractiveQuestion::LaravelQueueWorker { default: true });
    }

    // --- Non-standard port confirmation ---
    let standard_ports: &[u16] = &[80, 443, 3000, 8080];
    for svc in &analysis.services {
        if let Some(&port) = svc.exposed_ports.first() {
            if !standard_ports.contains(&port) {
                questions.push(InteractiveQuestion::ConfirmServicePort {
                    service_name: svc.name.clone(),
                    default_port: port,
                });
            }
        }
    }

    questions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::path::PathBuf;

    fn make_service(name: &str, fw: Framework, ports: Vec<u16>) -> Service {
        Service {
            name: name.into(),
            path: PathBuf::from(format!("/project/{name}")),
            package_name: None,
            language: Language::NodeJs,
            framework: fw,
            package_manager: PackageManager::Npm,
            runtime_version: Some("20".into()),
            entrypoint: None,
            exposed_ports: ports,
            env_vars: vec![],
            service_type: ServiceType::Api,
            build_command: None,
            start_command: None,
            is_monorepo: false,
        }
    }

    fn make_analysis(services: Vec<Service>, infra: Vec<InfraService>) -> ProjectAnalysis {
        ProjectAnalysis {
            root_path: PathBuf::from("/project"),
            is_monorepo: false,
            workspace_tool: None,
            services,
            detected_infrastructures: infra,
            warnings: vec![],
        }
    }

    #[test]
    fn infra_questions_for_detected_services() {
        let infra = vec![InfraService {
            kind: InfraKind::Postgres,
            name: "postgres".into(),
            image: "postgres:16-alpine".into(),
            port: 5432,
            env_vars: vec![],
            is_attached_to_compose: true,
            source: InfraSource::EnvVar("DATABASE_URL".into()),
        }];
        let analysis = make_analysis(vec![], infra);
        let questions = build_questions(&analysis);

        let attach_pg = questions.iter().any(|q| {
            matches!(
                q,
                InteractiveQuestion::AttachInfra {
                    kind: InfraKind::Postgres,
                    default: true,
                    ..
                }
            )
        });
        assert!(attach_pg);
    }

    #[test]
    fn no_questions_for_empty_project() {
        let analysis = make_analysis(vec![], vec![]);
        let questions = build_questions(&analysis);
        assert!(questions.is_empty());
    }

    #[test]
    fn prisma_question_for_prisma_source() {
        let infra = vec![InfraService {
            kind: InfraKind::Postgres,
            name: "postgres".into(),
            image: "postgres:16-alpine".into(),
            port: 5432,
            env_vars: vec![],
            is_attached_to_compose: true,
            source: InfraSource::PrismaSchema,
        }];
        let analysis = make_analysis(vec![], infra);
        let questions = build_questions(&analysis);

        let has_prisma_q = questions
            .iter()
            .any(|q| matches!(q, InteractiveQuestion::RunPrismaMigrations { .. }));
        assert!(has_prisma_q);
    }

    #[test]
    fn laravel_queue_worker_question() {
        let svc = make_service("api", Framework::Laravel, vec![8000]);
        let analysis = make_analysis(vec![svc], vec![]);
        let questions = build_questions(&analysis);

        let has_qw = questions
            .iter()
            .any(|q| matches!(q, InteractiveQuestion::LaravelQueueWorker { .. }));
        assert!(has_qw);
    }

    #[test]
    fn non_standard_port_question() {
        let svc = make_service("api", Framework::FastApi, vec![9000]);
        let analysis = make_analysis(vec![svc], vec![]);
        let questions = build_questions(&analysis);

        let port_q = questions.iter().find(|q| {
            matches!(
                q,
                InteractiveQuestion::ConfirmServicePort {
                    service_name,
                    default_port: 9000,
                    ..
                } if service_name == "api"
            )
        });
        assert!(port_q.is_some());
    }

    #[test]
    fn standard_port_no_question() {
        let svc = make_service("web", Framework::NextJs, vec![3000]);
        let analysis = make_analysis(vec![svc], vec![]);
        let questions = build_questions(&analysis);

        let has_port_q = questions
            .iter()
            .any(|q| matches!(q, InteractiveQuestion::ConfirmServicePort { .. }));
        assert!(!has_port_q);
    }

    #[test]
    fn non_attached_infra_no_question() {
        let infra = vec![InfraService {
            kind: InfraKind::Sqlite,
            name: "sqlite".into(),
            image: "alpine:3.20".into(),
            port: 0,
            env_vars: vec![],
            is_attached_to_compose: false,
            source: InfraSource::ManifestDependency("rusqlite".into()),
        }];
        let analysis = make_analysis(vec![], infra);
        let questions = build_questions(&analysis);

        let has_attach = questions
            .iter()
            .any(|q| matches!(q, InteractiveQuestion::AttachInfra { .. }));
        assert!(!has_attach);
    }
}
