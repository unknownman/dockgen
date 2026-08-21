mod analyzer;
mod cli;
mod detector;
mod generator;
mod interactive;
mod models;
mod templates;

use std::path::Path;

use anyhow::Context;
use clap::Parser;
use colored::Colorize;

use cli::Cli;

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{} {e:#}", "✖ Error:".red().bold());
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

fn run() -> anyhow::Result<()> {
    // --- Step 1: CLI parsing & tracing setup ---
    let cli = Cli::parse();

    let quiet = cli.quiet || cli.json;

    if !quiet {
        if cli.verbose {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .init();
        }
    }

    // --- Step 2: Path resolution ---
    let target_path = cli.get_target_path();

    // --- Step 3: Analysis ---
    let lang_override = cli.parse_language_override();
    let fw_override = cli.parse_framework_override();
    let services_filter: Vec<String> = cli.services.clone().unwrap_or_default();

    let analysis = detector::analyze_full_project(
        &target_path,
        lang_override.as_ref(),
        fw_override.as_ref(),
        &services_filter,
    )?;

    // --- Step 4: JSON output mode ---
    if cli.json {
        let mut config = cli.to_generation_config();
        // Non-interactive defaults for JSON mode.
        if analysis
            .detected_infrastructures
            .iter()
            .any(|i| i.is_attached_to_compose)
        {
            config.emit_compose = true;
        }
        let answers = interactive::run_interactive_wizard(&analysis, &mut config)?;
        config.interactive_answers = Some(answers);
        let files = generator::generate_all_files(&analysis, &config)?;

        let output = serde_json::json!({
            "analysis": analysis,
            "files": files,
            "warnings": analysis.warnings,
        });

        println!(
            "{}",
            serde_json::to_string_pretty(&output).context("failed to serialize JSON output")?
        );
        std::process::exit(0);
    }

    // --- Step 5: Terminal banner & interactive report ---
    if !quiet {
        print_banner();
        print_analysis_report(&analysis);
    }

    // --- Step 6: Code generation & safe write ---
    let mut config = cli.to_generation_config();

    // --- Step 6a: Interactive wizard (Phase 2) ---
    if config.interactive || config.assume_yes {
        let answers = interactive::run_interactive_wizard(&analysis, &mut config)?;
        config.interactive_answers = Some(answers);
    }

    let files = generator::generate_all_files(&analysis, &config)?;

    let output_dir = cli.output_dir.as_deref().unwrap_or(&target_path);

    generator::write_generated_files(&files, output_dir, config.dry_run)?;

    // --- Step 7: Summary & next steps ---
    if !quiet {
        print_summary(&files, &config, &target_path);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn print_banner() {
    let banner = format!(
        r#"
  ____            _  _
 |  _ \  ___   __| || |_
 | | | |/ _ \ / _` || __|
 | |_| | (_) | (_| || |_
 |____/ \___/ \__,_|\__|  v{}"#,
        env!("CARGO_PKG_VERSION")
    );
    println!("{}", banner.cyan().bold());
    println!();
}

fn print_analysis_report(analysis: &models::ProjectAnalysis) {
    // Project type
    if analysis.is_monorepo {
        let tool = analysis.workspace_tool.as_deref().unwrap_or("unknown");
        println!(
            "  {} {} ({})",
            "Project Type:".bold(),
            "Monorepo".green().bold(),
            tool
        );
    } else {
        println!(
            "  {} {}",
            "Project Type:".bold(),
            "Single Service".green().bold()
        );
    }

    // Services table
    println!(
        "\n  {}",
        format!("Discovered {} service(s):", analysis.services.len()).bold()
    );
    println!(
        "  {:<20} {:<12} {:<18} {:<10} {}",
        "Name".underline(),
        "Language".underline(),
        "Framework".underline(),
        "Ports".underline(),
        "Path".underline()
    );

    for svc in &analysis.services {
        let ports = if svc.exposed_ports.is_empty() {
            "-".to_string()
        } else {
            svc.exposed_ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        println!(
            "  {:<20} {:<12} {:<18} {:<10} {}",
            svc.name,
            svc.language.to_string(),
            svc.framework.to_string(),
            ports,
            svc.path.display()
        );
    }

    // Warnings
    if !analysis.warnings.is_empty() {
        println!();
        for warn in &analysis.warnings {
            println!("  {} {}", "⚠ Warning:".yellow().bold(), warn.yellow());
        }
    }

    println!();
}

fn print_summary(
    files: &[models::GeneratedFile],
    config: &models::GenerationConfig,
    target_path: &Path,
) {
    println!("  {}", format!("Generated {} file(s):", files.len()).bold());

    for file in files {
        println!(
            "    {} {} ({})",
            "✓".green().bold(),
            file.relative_path.display().to_string().green(),
            file.description
        );
    }

    println!();

    if config.dry_run {
        println!(
            "  {}",
            "Dry-run mode: no files written to disk.".yellow().italic()
        );
    } else {
        println!("  {}", "Files written successfully.".green().bold());
        println!();
        println!("  {}:", "Next steps".bold());

        if config.emit_compose {
            println!(
                "    docker compose -f {}/docker-compose.yml up --build",
                target_path.display()
            );
        } else {
            for file in files {
                if file.relative_path.file_name() == Some(std::ffi::OsStr::new("Dockerfile")) {
                    let dir = file
                        .relative_path
                        .parent()
                        .unwrap_or(std::path::Path::new("."));
                    let tag = if dir.as_os_str() == "." {
                        "my-app".to_string()
                    } else {
                        dir.to_string_lossy().replace('/', "-")
                    };
                    println!(
                        "    docker build -t {tag} -f {}/{} {}",
                        target_path.display(),
                        file.relative_path.display(),
                        if dir.as_os_str() == "." {
                            target_path.display().to_string()
                        } else {
                            format!("{}/{}", target_path.display(), dir.display())
                        }
                    );
                }
            }
        }
    }

    println!();
}
