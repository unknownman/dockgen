use anyhow::{Context, Result};
use inquire::Confirm;

use crate::models::{InfraKind, InfraService};

use super::questions::InteractiveQuestion;

// ---------------------------------------------------------------------------
// TTY detection
// ---------------------------------------------------------------------------

/// Returns `true` if stdin is connected to an interactive terminal (TTY).
pub fn is_terminal() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdin())
}

// ---------------------------------------------------------------------------
// Prompt dispatch
// ---------------------------------------------------------------------------

/// Present a single [`InteractiveQuestion`] to the user via the terminal and
/// return the resolved answer as an [`InfraAnswer`].
///
/// # Errors
///
/// Returns an error if the underlying `inquire` prompt fails (e.g. broken
/// pipe, interrupted input).
pub fn ask_question(question: &InteractiveQuestion, _infra: &InfraService) -> Result<InfraAnswer> {
    match question {
        InteractiveQuestion::AttachInfra { kind, default } => {
            let message = format!(
                "Add {} service to docker-compose.yml?",
                format_infra_label(*kind)
            );
            let answer = ask_confirm(&message, *default)?;
            Ok(InfraAnswer::AttachInfra {
                kind: *kind,
                attach: answer,
            })
        }
        InteractiveQuestion::RunPrismaMigrations { default } => {
            let answer = ask_confirm(
                "Run `prisma migrate deploy` on container startup?",
                *default,
            )?;
            Ok(InfraAnswer::RunPrismaMigrations(answer))
        }
        InteractiveQuestion::LaravelQueueWorker { default } => {
            let answer = ask_confirm("Generate a background queue worker service?", *default)?;
            Ok(InfraAnswer::LaravelQueueWorker(answer))
        }
        InteractiveQuestion::ConfirmServicePort {
            service_name,
            default_port,
        } => {
            let message = format!(
                "Confirm exposed port for service '{}' (default: {}):",
                service_name, default_port
            );
            let input = ask_port(&message, *default_port)?;
            Ok(InfraAnswer::ConfirmServicePort {
                service_name: service_name.clone(),
                port: input,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Answer type (internal to prompts — forwarded to orchestrator)
// ---------------------------------------------------------------------------

/// Resolved answer from a single prompt.
#[derive(Debug, Clone)]
pub(crate) enum InfraAnswer {
    AttachInfra { kind: InfraKind, attach: bool },
    RunPrismaMigrations(bool),
    LaravelQueueWorker(bool),
    ConfirmServicePort { service_name: String, port: u16 },
}

// ---------------------------------------------------------------------------
// Low-level prompt wrappers
// ---------------------------------------------------------------------------

/// Ask a yes/no confirmation question.
fn ask_confirm(message: &str, default: bool) -> Result<bool> {
    let answer = Confirm::new(message)
        .with_default(default)
        .prompt()
        .context("interactive prompt failed")?;
    Ok(answer)
}

/// Ask for a port number with a default, returning the user's input or the
/// default if the input is empty.
fn ask_port(message: &str, default_port: u16) -> Result<u16> {
    let raw = inquire::Text::new(message)
        .with_default(&default_port.to_string())
        .prompt()
        .context("interactive port prompt failed")?;

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default_port);
    }

    trimmed
        .parse::<u16>()
        .with_context(|| format!("invalid port number: '{trimmed}'"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format an [`InfraKind`] into a human-friendly label for prompts.
fn format_infra_label(kind: InfraKind) -> String {
    match kind {
        InfraKind::Postgres => "PostgreSQL".to_string(),
        InfraKind::Mysql => "MySQL".to_string(),
        InfraKind::Redis => "Redis".to_string(),
        InfraKind::Mongo => "MongoDB".to_string(),
        InfraKind::RabbitMq => "RabbitMQ".to_string(),
        InfraKind::Kafka => "Apache Kafka".to_string(),
        InfraKind::Sqlite => "SQLite".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_infra_labels() {
        assert_eq!(format_infra_label(InfraKind::Postgres), "PostgreSQL");
        assert_eq!(format_infra_label(InfraKind::Mysql), "MySQL");
        assert_eq!(format_infra_label(InfraKind::Redis), "Redis");
        assert_eq!(format_infra_label(InfraKind::Mongo), "MongoDB");
        assert_eq!(format_infra_label(InfraKind::RabbitMq), "RabbitMQ");
        assert_eq!(format_infra_label(InfraKind::Kafka), "Apache Kafka");
        assert_eq!(format_infra_label(InfraKind::Sqlite), "SQLite");
    }

    #[test]
    fn is_terminal_does_not_panic() {
        // Just ensure it returns a value without crashing.
        let _ = is_terminal();
    }

    #[test]
    fn infra_answer_debug() {
        let ans = InfraAnswer::AttachInfra {
            kind: InfraKind::Postgres,
            attach: true,
        };
        let debug = format!("{ans:?}");
        assert!(debug.contains("Postgres"));
        assert!(debug.contains("true"));
    }
}
