pub mod compose;
pub mod dockerfile;
pub mod dockerignore;

use std::path::Path;

use anyhow::{Context, Result};

use crate::models::{GeneratedFile, GenerationConfig, ProjectAnalysis};
use crate::templates::create_tera_engine;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Orchestrate the full generation pipeline.
///
/// 1. Initialise the Tera template engine.
/// 2. Generate Dockerfiles, `.dockerignore` files, and (optionally) a
///    `docker-compose.yml`.
/// 3. Aggregate and sort all [`GeneratedFile`]s deterministically by
///    `relative_path`.
pub fn generate_all_files(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
) -> Result<Vec<GeneratedFile>> {
    let tera = create_tera_engine().context("failed to initialise template engine")?;

    let mut files = Vec::new();

    // --- Dockerfiles ---
    let dockerfiles = dockerfile::generate_dockerfiles(analysis, config, &tera)
        .context("dockerfile generation failed")?;
    files.extend(dockerfiles);

    // --- .dockerignore files ---
    let dockerignores = dockerignore::generate_dockerignores(analysis, config, &tera)
        .context("dockerignore generation failed")?;
    files.extend(dockerignores);

    // --- docker-compose.yml (optional) ---
    if let Some(compose) = compose::generate_docker_compose(analysis, config, &tera)
        .context("compose generation failed")?
    {
        files.push(compose);
    }

    // Deterministic sort by output path.
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(files)
}

/// Write generated files to disk, or display them in a formatted dry-run.
///
/// When `dry_run` is `true` nothing is written; files are printed to stdout
/// with clear banners.
pub fn write_generated_files(
    files: &[GeneratedFile],
    output_base_dir: &Path,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        print_dry_run(files);
        return Ok(());
    }

    for file in files {
        let dest = output_base_dir.join(&file.relative_path);

        // Create parent directories.
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        std::fs::write(&dest, &file.content)
            .with_context(|| format!("failed to write {}", dest.display()))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Dry-run display
// ---------------------------------------------------------------------------

fn print_dry_run(files: &[GeneratedFile]) {
    use std::io::Write;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    for file in files {
        let sep = "─".repeat(60);
        let _ = writeln!(handle, "\n╔{sep}╗");
        let _ = writeln!(
            handle,
            "║ {} ({})",
            file.relative_path.display(),
            file.description
        );
        let _ = writeln!(handle, "╚{sep}╝");
        let _ = writeln!(handle, "{}", file.content);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::path::{Path, PathBuf};

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

    #[test]
    fn generate_all_single_service() {
        let svc = make_service("api", Language::Go, Framework::Gin);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        // Dockerfile + .dockerignore = 2
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("Dockerfile")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new(".dockerignore")));
    }

    #[test]
    fn generate_all_monorepo() {
        let svcs = vec![
            make_service("frontend", Language::NodeJs, Framework::NextJs),
            make_service("backend", Language::Go, Framework::Gin),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        // 2 Dockerfiles + 2 .dockerignores + 1 root .dockerignore = 5
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn generate_all_with_compose() {
        let svcs = vec![
            make_service("web", Language::NodeJs, Framework::Express),
            make_service("api", Language::Python, Framework::FastApi),
        ];
        let analysis = make_analysis(svcs, true);
        let config = GenerationConfig {
            emit_compose: true,
            ..default_config()
        };

        let files = generate_all_files(&analysis, &config).unwrap();
        // 2 Dockerfiles + 2 .dockerignores + 1 root .dockerignore + 1 compose = 6
        assert_eq!(files.len(), 6);
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("docker-compose.yml")));
    }

    #[test]
    fn files_sorted_by_relative_path() {
        let svcs = vec![
            make_service("zebra", Language::Go, Framework::GoGeneric),
            make_service("alpha", Language::Rust, Framework::RustGeneric),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        let paths: Vec<_> = files.iter().map(|f| &f.relative_path).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }

    #[test]
    fn write_files_dry_run_does_not_write() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let svc = make_service("api", Language::Go, Framework::GoGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        write_generated_files(&files, dir.path(), true).unwrap();

        // Directory should be empty — dry run doesn't write.
        assert!(dir.path().read_dir().unwrap().next().is_none());
    }

    #[test]
    fn write_files_to_disk() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let svc = make_service("api", Language::Go, Framework::GoGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        write_generated_files(&files, dir.path(), false).unwrap();

        assert!(dir.path().join("Dockerfile").exists());
        assert!(dir.path().join(".dockerignore").exists());
        let content = std::fs::read_to_string(dir.path().join("Dockerfile")).unwrap();
        assert!(content.contains("golang:"));
    }

    #[test]
    fn write_files_creates_parent_dirs() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let svcs = vec![
            make_service("frontend", Language::NodeJs, Framework::NextJs),
            make_service("backend", Language::Go, Framework::Gin),
        ];
        let analysis = make_analysis(svcs, true);
        let config = default_config();

        let files = generate_all_files(&analysis, &config).unwrap();
        write_generated_files(&files, dir.path(), false).unwrap();

        assert!(dir.path().join("frontend/Dockerfile").exists());
        assert!(dir.path().join("backend/Dockerfile").exists());
    }
}
