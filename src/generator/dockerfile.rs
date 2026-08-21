use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tera::Tera;

use crate::models::{
    Framework, GeneratedFile, GenerationConfig, Language, PackageManager, ProjectAnalysis, Service,
};
use crate::templates::resolve_dockerfile_template;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate `Dockerfile` contents for every service in the project.
///
/// * **Single-service** – a single `Dockerfile` is placed at the output root.
/// * **Monorepo** – a separate `Dockerfile` is placed inside each service's
///   relative directory.
/// * **`force_single`** – generates `Dockerfile.<service_name>` per service to
///   avoid overwriting when multiple services are collapsed to the root.
pub fn generate_dockerfiles(
    analysis: &ProjectAnalysis,
    config: &GenerationConfig,
    tera: &Tera,
) -> Result<Vec<GeneratedFile>> {
    if analysis.services.is_empty() {
        anyhow::bail!("no services to generate Dockerfiles for");
    }

    let is_single_service = !analysis.is_monorepo || analysis.services.len() == 1;
    let force_single = config.force_single && analysis.services.len() > 1;

    let mut files = Vec::new();

    for (idx, service) in analysis.services.iter().enumerate() {
        let ctx = build_dockerfile_context(service, config, idx);
        let tpl_path = resolve_dockerfile_template(&service.language, &service.framework);

        let content = tera.render(tpl_path, &ctx).with_context(|| {
            format!("failed to render Dockerfile for service '{}'", service.name)
        })?;

        let relative_path = if is_single_service {
            "Dockerfile".into()
        } else if force_single {
            // force_single always produces distinct named files to prevent
            // overwrites when multiple services are written to the same root.
            std::path::PathBuf::from(format!("Dockerfile.{}", service.name))
        } else {
            std::path::PathBuf::from(to_slash_path(&Path::new(&service.name).join("Dockerfile")))
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
// Path helpers
// ---------------------------------------------------------------------------

/// Convert a path to a forward-slash string, ensuring cross-platform
/// consistency in generated Docker-related paths.
pub fn to_slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_string()
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

    // Base image variant — default to "alpine" for smallest images.
    let base_variant = config
        .base_image_override
        .map(|v| v.to_string())
        .unwrap_or_else(|| "alpine".to_string());
    ctx.insert("base_image_variant", &base_variant);

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

    // Build tool for Java (maven vs gradle).
    let build_tool = match service.package_manager {
        PackageManager::Gradle => "gradle",
        _ => "maven",
    };
    ctx.insert("build_tool", &build_tool);

    // Hybrid asset build: detect if a Node.js frontend build step is needed.
    // When a backend service (PHP, Python, Ruby, etc.) also has a
    // `package.json`, the templates can optionally run a Node build stage
    // for Vite/Inertia/esbuild assets.
    let has_frontend_assets =
        service.path.join("package.json").is_file() && !is_node_language(&service.language);
    ctx.insert("has_frontend_assets", &has_frontend_assets);

    // Node.js package manager string for hybrid build stages.
    let node_pm = match &service.package_manager {
        PackageManager::Pnpm => "pnpm",
        PackageManager::Yarn => "yarn",
        PackageManager::Bun => "bun",
        _ => "npm",
    };
    ctx.insert("node_pm", &node_pm.to_string());

    // Binary / assembly name — preferred order: package_name → entrypoint → name.
    let bin_name = resolve_bin_name(service);
    ctx.insert("bin_name", &bin_name);

    // Assembly name for .NET templates (same resolution logic).
    ctx.insert("assembly_name", &bin_name);

    // Python short version (e.g. "3.11" from "3.11.9") — computed in Rust
    // so templates never need fragile Tera split/slice chains.
    let py_short_version = compute_python_short_version(version);
    ctx.insert("py_short_version", &py_short_version);

    // Node.js package manager run prefix for build/start commands.
    let pm_run_prefix = compute_pm_run_prefix(&service.package_manager);
    ctx.insert("pm_run_prefix", &pm_run_prefix);

    // Framework-specific entrypoint and start configuration.
    let (entrypoint_file, entrypoint_dir) = framework_entrypoint(&service.framework);
    ctx.insert("entrypoint_file", &entrypoint_file);
    ctx.insert("entrypoint_dir", &entrypoint_dir);

    // Environment variables — sorted by key for deterministic output.
    let mut sorted_env: Vec<&(String, String)> = service.env_vars.iter().collect();
    sorted_env.sort_by(|a, b| a.0.cmp(&b.0));

    let env_map: Vec<HashMap<&str, &str>> = sorted_env
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

/// Returns `true` if the language is a Node.js variant.
fn is_node_language(lang: &Language) -> bool {
    matches!(lang, Language::NodeJs)
}

/// Returns `(entrypoint_file, entrypoint_dir)` for framework-specific
/// runtime entry points.
fn framework_entrypoint(fw: &Framework) -> (String, String) {
    match fw {
        // Node.js SSR frameworks
        Framework::SvelteKit => ("index.js".into(), "build".into()),
        Framework::Remix => ("index.js".into(), "build/server".into()),
        Framework::Astro => ("entry.mjs".into(), "dist/server".into()),
        Framework::Fastify => ("index.js".into(), "dist".into()),
        // Node.js defaults
        Framework::NextJs => ("server.js".into(), ".".into()),
        Framework::Nuxt => ("index.mjs".into(), ".output/server".into()),
        Framework::NestJs => ("main.js".into(), "dist".into()),
        Framework::Express | Framework::NodeGeneric => ("index.js".into(), "dist".into()),
        // Python
        Framework::FastApi | Framework::Starlette | Framework::Litestar => {
            ("app".into(), ".".into())
        }
        Framework::Django => ("wsgi.py".into(), "config".into()),
        Framework::Flask | Framework::PythonGeneric => ("app.py".into(), ".".into()),
        // Go
        Framework::Gin
        | Framework::Echo
        | Framework::Fiber
        | Framework::Chi
        | Framework::GoGeneric => ("server".into(), ".".into()),
        // Rust
        Framework::Axum
        | Framework::ActixWeb
        | Framework::Rocket
        | Framework::Warp
        | Framework::RustGeneric => ("server".into(), ".".into()),
        // Java
        Framework::SpringBoot
        | Framework::Quarkus
        | Framework::Micronaut
        | Framework::JavaGeneric => ("app.jar".into(), ".".into()),
        // PHP
        Framework::Laravel | Framework::Symfony | Framework::PhpGeneric => {
            ("index.php".into(), ".".into())
        }
        // .NET
        Framework::AspNetCore | Framework::DotNetGeneric => ("app.dll".into(), ".".into()),
        // Ruby
        Framework::Rails | Framework::Sinatra | Framework::RubyGeneric => {
            ("config.ru".into(), ".".into())
        }
        // Generic fallback
        Framework::Generic => ("server".into(), ".".into()),
    }
}

/// Sensible default runtime version strings per language family.
fn default_runtime_version(lang: &Language) -> String {
    match lang {
        Language::NodeJs => "20".into(),
        Language::Python => "3.11".into(),
        Language::Go => "1.22".into(),
        Language::Rust => "1.78".into(),
        Language::Java => "21".into(),
        Language::Php => "8.2".into(),
        Language::DotNet => "8.0".into(),
        Language::Ruby => "3.2".into(),
        Language::Unknown(_) => "latest".into(),
    }
}

/// Resolve the binary / assembly name for a service.
///
/// Preferred order: `package_name` → `entrypoint` (stem) → `name`.
fn resolve_bin_name(service: &Service) -> String {
    if let Some(ref pkg) = service.package_name {
        return pkg.clone();
    }
    if let Some(ref ep) = service.entrypoint {
        // Strip path and extension to get the stem (e.g. "cmd/server" → "server").
        let stem = std::path::Path::new(ep)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(ep);
        return stem.to_string();
    }
    service.name.clone()
}

/// Compute a two-component Python version string suitable for site-packages paths.
///
/// `compute_python_short_version("3.11.9")` → `"3.11"`
/// `compute_python_short_version("3.11")`   → `"3.11"`
fn compute_python_short_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        version.to_string()
    }
}

/// Compute the package-manager run prefix for Node.js services.
///
/// Used in templates like `{{ pm_run_prefix }} build` to produce
/// `npm run build`, `pnpm build`, `yarn build`, or `bun run build`.
fn compute_pm_run_prefix(pm: &PackageManager) -> &'static str {
    match pm {
        PackageManager::Pnpm => "pnpm",
        PackageManager::Yarn => "yarn",
        PackageManager::Bun => "bun run",
        _ => "npm run",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::templates::create_tera_engine;
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
            detected_infrastructures: vec![],
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
    fn force_single_mixed_lang_produces_named_files() {
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
        // force_single always produces distinct named files.
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("Dockerfile.a")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("Dockerfile.b")));
    }

    #[test]
    fn force_single_same_lang_produces_named_files() {
        let svcs = vec![
            make_service("a", Language::Go, Framework::Gin),
            make_service("b", Language::Go, Framework::Echo),
        ];
        let analysis = make_analysis(svcs, true);
        let config = GenerationConfig {
            force_single: true,
            ..default_config()
        };
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert_eq!(files.len(), 2);
        // force_single always produces distinct named files, even for same lang.
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("Dockerfile.a")));
        assert!(files
            .iter()
            .any(|f| f.relative_path == Path::new("Dockerfile.b")));
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

    #[test]
    fn build_tool_gradle_injected() {
        let mut svc = make_service("api", Language::Java, Framework::SpringBoot);
        svc.package_manager = PackageManager::Gradle;
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert!(files[0].content.contains("gradlew"));
    }

    #[test]
    fn build_tool_maven_default() {
        let svc = make_service("api", Language::Java, Framework::SpringBoot);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        assert!(files[0].content.contains("mvnw"));
    }

    #[test]
    fn bun_lockfile_detected_in_node_deps() {
        let svc = make_service("web", Language::NodeJs, Framework::NodeGeneric);
        let analysis = make_analysis(vec![svc], false);
        let config = default_config();
        let tera = tera_engine();

        let files = generate_dockerfiles(&analysis, &config, &tera).unwrap();
        // The node/generic template should include bun.lockb detection.
        assert!(files[0].content.contains("bun.lockb") || files[0].content.contains("bun.lock"));
    }

    #[test]
    fn framework_entrypoint_sveltekit() {
        let (file, dir) = framework_entrypoint(&Framework::SvelteKit);
        assert_eq!(file, "index.js");
        assert_eq!(dir, "build");
    }

    #[test]
    fn framework_entrypoint_astro() {
        let (file, dir) = framework_entrypoint(&Framework::Astro);
        assert_eq!(file, "entry.mjs");
        assert_eq!(dir, "dist/server");
    }

    #[test]
    fn framework_entrypoint_remix() {
        let (file, dir) = framework_entrypoint(&Framework::Remix);
        assert_eq!(file, "index.js");
        assert_eq!(dir, "build/server");
    }

    #[test]
    fn to_slash_path_normalizes_separators() {
        let p = Path::new("frontend").join("Dockerfile");
        assert_eq!(to_slash_path(&p), "frontend/Dockerfile");

        // On Windows the path would contain backslashes; to_slash_path
        // normalises them.  On Unix the path is already forward-slash but
        // the function is still a no-op passthrough.
        assert_eq!(to_slash_path(Path::new("Dockerfile")), "Dockerfile");
        assert_eq!(
            to_slash_path(&PathBuf::from("services").join("api").join("Dockerfile")),
            "services/api/Dockerfile"
        );
    }
}
