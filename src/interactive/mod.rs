pub mod prompts;
pub mod questions;

use std::collections::BTreeMap;

use anyhow::Result;
use colored::Colorize;

use crate::models::{GenerationConfig, InfraKind, InteractiveAnswers, ProjectAnalysis};

use self::prompts::{is_terminal, InfraAnswer};
use self::questions::{build_questions, InteractiveQuestion};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run the Phase 2 interactive wizard, collecting user answers and mutating
/// `config` with the results.
///
/// When `config.assume_yes` is `true`, all questions are auto-resolved to
/// conservative defaults without prompting. When `is_terminal()` returns
/// `false` (non-TTY stdin), the wizard silently falls back to defaults as
/// well.
///
/// # Errors
///
/// Returns an error only on I/O failures or user interruption (Ctrl-C).
pub fn run_interactive_wizard(
    analysis: &ProjectAnalysis,
    config: &mut GenerationConfig,
) -> Result<InteractiveAnswers> {
    let questions = build_questions(analysis);

    let auto_mode = config.assume_yes || !is_terminal();

    if questions.is_empty() || auto_mode {
        return apply_defaults(analysis, config);
    }

    // Interactive path: present each question.
    println!("\n  {}", "Interactive Configuration".cyan().bold());
    println!(
        "  {}\n",
        "Answer the following to customise your docker-compose setup:".dimmed()
    );

    let answers = collect_answers(&questions, analysis, config)?;
    Ok(answers)
}

// ---------------------------------------------------------------------------
// Defaults (non-interactive / assume-yes)
// ---------------------------------------------------------------------------

/// Populate `InteractiveAnswers` with conservative defaults:
/// - All detected compose-eligible infrastructure is included.
/// - Prisma migrations: enabled.
/// - Laravel queue worker: enabled.
/// - No custom port overrides.
fn apply_defaults(
    analysis: &ProjectAnalysis,
    config: &mut GenerationConfig,
) -> Result<InteractiveAnswers> {
    let include_infra: Vec<InfraKind> = analysis
        .detected_infrastructures
        .iter()
        .filter(|i| i.is_attached_to_compose)
        .map(|i| i.kind)
        .collect();

    // If any infrastructure is selected, ensure compose is emitted.
    if !include_infra.is_empty() {
        config.emit_compose = true;
    }

    let answers = InteractiveAnswers {
        include_infra_in_compose: include_infra,
        run_prisma_migrations: Some(true),
        create_queue_worker: Some(true),
        custom_service_ports: BTreeMap::new(),
    };

    config.interactive_answers = Some(answers.clone());
    Ok(answers)
}

// ---------------------------------------------------------------------------
// Interactive collection
// ---------------------------------------------------------------------------

/// Walk through each question, present it to the user, and accumulate the
/// answers.
fn collect_answers(
    questions: &[InteractiveQuestion],
    analysis: &ProjectAnalysis,
    config: &mut GenerationConfig,
) -> Result<InteractiveAnswers> {
    let mut include_infra: Vec<InfraKind> = Vec::new();
    let mut run_prisma_migrations: Option<bool> = None;
    let mut create_queue_worker: Option<bool> = None;
    let mut custom_ports: BTreeMap<String, u16> = BTreeMap::new();

    for question in questions {
        // Resolve the InfraService context for infra-related questions.
        let dummy_infra = crate::models::InfraService {
            kind: InfraKind::Postgres,
            name: "postgres".into(),
            image: "postgres:16-alpine".into(),
            port: 5432,
            env_vars: vec![],
            is_attached_to_compose: true,
            source: crate::models::InfraSource::ManualOverride,
        };

        let infra_ctx = match question {
            InteractiveQuestion::AttachInfra { kind, .. } => analysis
                .detected_infrastructures
                .iter()
                .find(|i| i.kind == *kind)
                .unwrap_or(&dummy_infra),
            _ => &dummy_infra,
        };

        let answer = prompts::ask_question(question, infra_ctx)?;

        match answer {
            InfraAnswer::AttachInfra { kind, attach } => {
                if attach {
                    include_infra.push(kind);
                }
            }
            InfraAnswer::RunPrismaMigrations(val) => {
                run_prisma_migrations = Some(val);
            }
            InfraAnswer::LaravelQueueWorker(val) => {
                create_queue_worker = Some(val);
            }
            InfraAnswer::ConfirmServicePort { service_name, port } => {
                custom_ports.insert(service_name, port);
            }
        }
    }

    // If any infra was selected, ensure compose is emitted.
    if !include_infra.is_empty() {
        config.emit_compose = true;
    }

    let answers = InteractiveAnswers {
        include_infra_in_compose: include_infra,
        run_prisma_migrations,
        create_queue_worker,
        custom_service_ports: custom_ports,
    };

    config.interactive_answers = Some(answers.clone());
    Ok(answers)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::path::PathBuf;

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
    fn defaults_include_all_attached_infra() {
        let infra = vec![
            InfraService {
                kind: InfraKind::Postgres,
                name: "postgres".into(),
                image: "postgres:16-alpine".into(),
                port: 5432,
                env_vars: vec![],
                is_attached_to_compose: true,
                source: InfraSource::EnvVar("DATABASE_URL".into()),
            },
            InfraService {
                kind: InfraKind::Redis,
                name: "redis".into(),
                image: "redis:7-alpine".into(),
                port: 6379,
                env_vars: vec![],
                is_attached_to_compose: true,
                source: InfraSource::EnvVar("REDIS_URL".into()),
            },
            InfraService {
                kind: InfraKind::Sqlite,
                name: "sqlite".into(),
                image: "alpine:3.20".into(),
                port: 0,
                env_vars: vec![],
                is_attached_to_compose: false,
                source: InfraSource::ManifestDependency("rusqlite".into()),
            },
        ];

        let analysis = make_analysis(vec![], infra);
        let mut config = default_config();
        config.assume_yes = true;

        let answers = run_interactive_wizard(&analysis, &mut config).unwrap();

        assert_eq!(answers.include_infra_in_compose.len(), 2);
        assert!(answers
            .include_infra_in_compose
            .contains(&InfraKind::Postgres));
        assert!(answers.include_infra_in_compose.contains(&InfraKind::Redis));
        // SQLite is not attached — should not be included.
        assert!(!answers
            .include_infra_in_compose
            .contains(&InfraKind::Sqlite));
        // Prisma defaults to true.
        assert_eq!(answers.run_prisma_migrations, Some(true));
        // Queue worker defaults to true.
        assert_eq!(answers.create_queue_worker, Some(true));
        // No custom ports in defaults.
        assert!(answers.custom_service_ports.is_empty());
    }

    #[test]
    fn defaults_enable_compose_when_infra_present() {
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
        let mut config = default_config();
        config.assume_yes = true;

        let _ = run_interactive_wizard(&analysis, &mut config).unwrap();

        assert!(config.emit_compose);
    }

    #[test]
    fn empty_project_returns_default_answers() {
        let analysis = make_analysis(vec![], vec![]);
        let mut config = default_config();
        config.assume_yes = true;

        let answers = run_interactive_wizard(&analysis, &mut config).unwrap();

        assert!(answers.include_infra_in_compose.is_empty());
        assert_eq!(answers.run_prisma_migrations, Some(true));
        assert_eq!(answers.create_queue_worker, Some(true));
    }

    #[test]
    fn config_interactive_answers_populated() {
        let analysis = make_analysis(vec![], vec![]);
        let mut config = default_config();
        config.assume_yes = true;

        let _ = run_interactive_wizard(&analysis, &mut config).unwrap();

        assert!(config.interactive_answers.is_some());
    }
}
