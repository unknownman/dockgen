use std::path::PathBuf;

use clap::Parser;

use crate::models::{BaseImageVariant, Framework, GenerationConfig, Language};

// ---------------------------------------------------------------------------
// Cli
// ---------------------------------------------------------------------------

/// Smart minimal Dockerfile generator — languages, frameworks & multi-service
/// projects.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "dockgen",
    version,
    about = "Blazing fast Rust CLI for deterministic Dockerfile and .dockerignore generation",
    long_about = None,
    after_help = "EXAMPLES:\n  dockgen                          # auto-detect & generate\n  dockgen ./services/api -l rust -f axum\n  dockgen --compose --dry-run\n  dockgen -p 3000,8000 --json"
)]
pub struct Cli {
    /// Root path of the project to analyze.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Explicitly override detected language (e.g., nodejs, python, go, rust,
    /// java, php, dotnet, ruby).
    #[arg(short = 'l', long = "lang")]
    pub lang: Option<String>,

    /// Explicitly override detected framework (e.g., nextjs, fastapi, gin,
    /// axum, etc.).
    #[arg(short = 'f', long = "fw")]
    pub fw: Option<String>,

    /// Filter specific services/directories to generate Dockerfiles for.
    /// Accepts comma-separated values or multiple invocations.
    #[arg(
        short = 's',
        long = "services",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub services: Option<Vec<String>>,

    /// Force generation of a single unified multi-stage Dockerfile.
    #[arg(long)]
    pub single: bool,

    /// Base image variant preference.
    #[arg(short = 'b', long = "base", value_enum)]
    pub base: Option<BaseImageVariant>,

    /// Generate docker-compose.yml file.
    #[arg(short = 'c', long = "compose")]
    pub compose: bool,

    /// Override exposed container ports in order of services. Accepts
    /// comma-separated values or multiple invocations.
    #[arg(
        short = 'p',
        long = "port",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub port: Option<Vec<String>>,

    /// Print generated Dockerfile(s) and configs to stdout without writing to
    /// disk.
    #[arg(long)]
    pub dry_run: bool,

    /// Output analysis and generation summary strictly as JSON.
    #[arg(long)]
    pub json: bool,

    /// Custom directory to write generated files.
    #[arg(short = 'o', long = "output-dir")]
    pub output_dir: Option<PathBuf>,

    /// Enable verbose tracing/debug output.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Suppress all terminal logs except errors and outputs.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

impl Cli {
    /// Converts parsed CLI arguments into the domain [`GenerationConfig`].
    pub fn to_generation_config(&self) -> GenerationConfig {
        GenerationConfig {
            base_image_override: self.base,
            port_overrides: self.parse_port_overrides(),
            force_single: self.single,
            dry_run: self.dry_run,
            emit_compose: self.compose,
            output_dir: self.output_dir.clone(),
        }
    }

    /// Converts the `--lang` string into a [`Language`] enum variant.
    ///
    /// Matching is case-insensitive and supports common aliases (`node`, `js`,
    /// `ts`, `golang`, `py`, `rs`, `csharp`, `cs`, `dotnet`, `rb`).
    pub fn parse_language_override(&self) -> Option<Language> {
        let raw = self.lang.as_deref()?;
        Some(normalize_lang(raw))
    }

    /// Converts the `--fw` string into a [`Framework`] enum variant.
    ///
    /// Matching is case-insensitive and supports common short aliases (`next`,
    /// `fastapi`, `spring`, `actix`, etc.).
    pub fn parse_framework_override(&self) -> Option<Framework> {
        let raw = self.fw.as_deref()?;
        Some(normalize_framework(raw))
    }

    /// Resolves and returns the target project path. If the path is relative,
    /// it is resolved against the current working directory.
    pub fn get_target_path(&self) -> PathBuf {
        if self.path.is_absolute() {
            self.path.clone()
        } else {
            let Ok(cwd) = std::env::current_dir() else {
                return self.path.clone();
            };
            cwd.join(&self.path)
        }
    }

    /// Parses the `--port` argument values into `u16` port numbers.
    fn parse_port_overrides(&self) -> Vec<u16> {
        self.port
            .as_ref()
            .map(|vals| vals.iter().filter_map(|s| s.parse::<u16>().ok()).collect())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Language normalization
// ---------------------------------------------------------------------------

fn normalize_lang(input: &str) -> Language {
    match input.to_ascii_lowercase().as_str() {
        // Node.js
        "node" | "nodejs" | "node.js" | "js" | "javascript" | "ts" | "typescript" => {
            Language::NodeJs
        }
        // Python
        "python" | "py" | "python3" => Language::Python,
        // Go
        "go" | "golang" => Language::Go,
        // Rust
        "rust" | "rs" => Language::Rust,
        // Java
        "java" | "jvm" | "kotlin" | "kt" | "scala" => Language::Java,
        // PHP
        "php" => Language::Php,
        // .NET
        "dotnet" | ".net" | "csharp" | "cs" | "fsharp" | "fs" => Language::DotNet,
        // Ruby
        "ruby" | "rb" => Language::Ruby,
        // Unknown / pass-through
        other => Language::Unknown(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Framework normalization
// ---------------------------------------------------------------------------

fn normalize_framework(input: &str) -> Framework {
    match input.to_ascii_lowercase().as_str() {
        // Node.js
        "next" | "nextjs" | "next.js" => Framework::NextJs,
        "nuxt" | "nuxtjs" | "nuxt.js" => Framework::Nuxt,
        "nest" | "nestjs" | "nest.js" => Framework::NestJs,
        "express" => Framework::Express,
        "fastify" => Framework::Fastify,
        "remix" => Framework::Remix,
        "sveltekit" | "svelte-kit" | "svelte" => Framework::SvelteKit,
        "astro" => Framework::Astro,
        "node" | "nodejs" | "node.js" => Framework::NodeGeneric,

        // Python
        "fastapi" | "fast-api" | "fast_api" => Framework::FastApi,
        "django" => Framework::Django,
        "flask" => Framework::Flask,
        "starlette" => Framework::Starlette,
        "litestar" => Framework::Litestar,
        "python" | "py" => Framework::PythonGeneric,

        // Go
        "gin" => Framework::Gin,
        "echo" => Framework::Echo,
        "fiber" => Framework::Fiber,
        "chi" => Framework::Chi,
        "go" | "golang" => Framework::GoGeneric,

        // Rust
        "actix" | "actixweb" | "actix-web" => Framework::ActixWeb,
        "axum" => Framework::Axum,
        "rocket" => Framework::Rocket,
        "warp" => Framework::Warp,
        "rust" | "rs" => Framework::RustGeneric,

        // Java
        "spring" | "springboot" | "spring-boot" | "spring_boot" => Framework::SpringBoot,
        "quarkus" => Framework::Quarkus,
        "micronaut" => Framework::Micronaut,
        "java" | "jvm" => Framework::JavaGeneric,

        // PHP
        "laravel" => Framework::Laravel,
        "symfony" => Framework::Symfony,
        "php" => Framework::PhpGeneric,

        // .NET
        "aspnet" | "aspnetcore" | "asp.net" | "asp-net-core" => Framework::AspNetCore,
        "dotnet" | ".net" | "csharp" => Framework::DotNetGeneric,

        // Ruby
        "rails" | "rubyonrails" | "ruby-on-rails" => Framework::Rails,
        "sinatra" => Framework::Sinatra,
        "ruby" | "rb" => Framework::RubyGeneric,

        // Fallback
        _ => Framework::Generic,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("dockgen").chain(args.iter().copied()))
            .expect("failed to parse CLI arguments")
    }

    // -- default arguments --------------------------------------------------

    #[test]
    fn defaults() {
        let cli = parse(&[]);
        assert_eq!(cli.path, PathBuf::from("."));
        assert!(cli.lang.is_none());
        assert!(cli.fw.is_none());
        assert!(cli.services.is_none());
        assert!(!cli.single);
        assert!(cli.base.is_none());
        assert!(!cli.compose);
        assert!(cli.port.is_none());
        assert!(!cli.dry_run);
        assert!(!cli.json);
        assert!(cli.output_dir.is_none());
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn default_generation_config() {
        let cli = parse(&[]);
        let cfg = cli.to_generation_config();
        assert_eq!(
            cfg,
            GenerationConfig {
                base_image_override: None,
                port_overrides: vec![],
                force_single: false,
                dry_run: false,
                emit_compose: false,
                output_dir: None,
            }
        );
    }

    // -- positional path ----------------------------------------------------

    #[test]
    fn positional_path() {
        let cli = parse(&["./my-project"]);
        assert_eq!(cli.path, PathBuf::from("./my-project"));
    }

    // -- boolean flags ------------------------------------------------------

    #[test]
    fn flags_single() {
        let cli = parse(&["--single"]);
        assert!(cli.single);
    }

    #[test]
    fn flags_dry_run() {
        let cli = parse(&["--dry-run"]);
        assert!(cli.dry_run);
    }

    #[test]
    fn flags_compose() {
        let cli = parse(&["-c"]);
        assert!(cli.compose);
    }

    #[test]
    fn flags_json() {
        let cli = parse(&["--json"]);
        assert!(cli.json);
    }

    #[test]
    fn flags_verbose() {
        let cli = parse(&["-v"]);
        assert!(cli.verbose);
    }

    #[test]
    fn flags_quiet() {
        let cli = parse(&["-q"]);
        assert!(cli.quiet);
    }

    #[test]
    fn flags_combined() {
        let cli = parse(&["--single", "--dry-run", "--compose", "--json"]);
        assert!(cli.single);
        assert!(cli.dry_run);
        assert!(cli.compose);
        assert!(cli.json);
    }

    // -- comma-separated services -------------------------------------------

    #[test]
    fn services_comma_separated() {
        let cli = parse(&["-s", "frontend,backend,worker"]);
        assert_eq!(
            cli.services,
            Some(vec![
                "frontend".to_string(),
                "backend".to_string(),
                "worker".to_string(),
            ])
        );
    }

    #[test]
    fn services_multiple_flags() {
        let cli = parse(&["--services", "frontend", "--services", "backend"]);
        assert_eq!(
            cli.services,
            Some(vec!["frontend".to_string(), "backend".to_string(),])
        );
    }

    #[test]
    fn services_mixed() {
        let cli = parse(&["-s", "frontend,backend", "--services", "worker"]);
        assert_eq!(
            cli.services,
            Some(vec![
                "frontend".to_string(),
                "backend".to_string(),
                "worker".to_string(),
            ])
        );
    }

    // -- comma-separated ports ----------------------------------------------

    #[test]
    fn ports_comma_separated() {
        let cli = parse(&["-p", "3000,8000"]);
        assert_eq!(
            cli.port,
            Some(vec!["3000".to_string(), "8000".to_string(),])
        );
    }

    #[test]
    fn ports_multiple_flags() {
        let cli = parse(&["--port", "3000", "--port", "8000"]);
        assert_eq!(
            cli.port,
            Some(vec!["3000".to_string(), "8000".to_string(),])
        );
    }

    #[test]
    fn port_overrides_parsed_to_u16() {
        let cli = parse(&["-p", "3000,8000"]);
        let cfg = cli.to_generation_config();
        assert_eq!(cfg.port_overrides, vec![3000, 8000]);
    }

    #[test]
    fn port_invalid_value_filtered() {
        let cli = parse(&["-p", "3000,notaport,8000"]);
        let cfg = cli.to_generation_config();
        assert_eq!(cfg.port_overrides, vec![3000, 8000]);
    }

    #[test]
    fn ports_empty() {
        let cli = parse(&[]);
        let cfg = cli.to_generation_config();
        assert!(cfg.port_overrides.is_empty());
    }

    // -- --lang override ----------------------------------------------------

    #[test]
    fn lang_override_exact() {
        let cli = parse(&["-l", "rust"]);
        assert_eq!(cli.parse_language_override(), Some(Language::Rust));
    }

    #[test]
    fn lang_override_case_insensitive() {
        let cli = parse(&["--lang", "PYTHON"]);
        assert_eq!(cli.parse_language_override(), Some(Language::Python));
    }

    #[test]
    fn lang_override_aliases() {
        let cases: &[(&str, Language)] = &[
            ("node", Language::NodeJs),
            ("js", Language::NodeJs),
            ("ts", Language::NodeJs),
            ("typescript", Language::NodeJs),
            ("golang", Language::Go),
            ("py", Language::Python),
            ("rs", Language::Rust),
            ("csharp", Language::DotNet),
            ("cs", Language::DotNet),
            ("rb", Language::Ruby),
            ("jvm", Language::Java),
            ("kotlin", Language::Java),
            ("php", Language::Php),
        ];

        for (alias, expected) in cases {
            let cli = parse(&["-l", alias]);
            assert_eq!(
                cli.parse_language_override(),
                Some(expected.clone()),
                "alias '{alias}' should map to {expected}"
            );
        }
    }

    #[test]
    fn lang_override_unknown() {
        let cli = parse(&["-l", "zig"]);
        assert_eq!(
            cli.parse_language_override(),
            Some(Language::Unknown("zig".to_string()))
        );
    }

    #[test]
    fn lang_override_none() {
        let cli = parse(&[]);
        assert_eq!(cli.parse_language_override(), None);
    }

    // -- --fw override ------------------------------------------------------

    #[test]
    fn fw_override_exact() {
        let cli = parse(&["-f", "axum"]);
        assert_eq!(cli.parse_framework_override(), Some(Framework::Axum));
    }

    #[test]
    fn fw_override_case_insensitive() {
        let cli = parse(&["--fw", "NEXTJS"]);
        assert_eq!(cli.parse_framework_override(), Some(Framework::NextJs));
    }

    #[test]
    fn fw_override_aliases() {
        let cases: &[(&str, Framework)] = &[
            ("next", Framework::NextJs),
            ("next.js", Framework::NextJs),
            ("nest", Framework::NestJs),
            ("fastapi", Framework::FastApi),
            ("fast-api", Framework::FastApi),
            ("spring", Framework::SpringBoot),
            ("springboot", Framework::SpringBoot),
            ("actix", Framework::ActixWeb),
            ("actix-web", Framework::ActixWeb),
            ("rails", Framework::Rails),
            ("aspnet", Framework::AspNetCore),
            ("asp-net-core", Framework::AspNetCore),
            ("svelte", Framework::SvelteKit),
            ("svelte-kit", Framework::SvelteKit),
            ("go", Framework::GoGeneric),
            ("rust", Framework::RustGeneric),
            ("python", Framework::PythonGeneric),
            ("java", Framework::JavaGeneric),
            ("php", Framework::PhpGeneric),
            ("dotnet", Framework::DotNetGeneric),
            ("ruby", Framework::RubyGeneric),
        ];

        for (alias, expected) in cases {
            let cli = parse(&["-f", alias]);
            assert_eq!(
                cli.parse_framework_override(),
                Some(expected.clone()),
                "alias '{alias}' should map to {expected}"
            );
        }
    }

    #[test]
    fn fw_override_unknown_falls_to_generic() {
        let cli = parse(&["-f", "something-random"]);
        assert_eq!(cli.parse_framework_override(), Some(Framework::Generic));
    }

    #[test]
    fn fw_override_none() {
        let cli = parse(&[]);
        assert_eq!(cli.parse_framework_override(), None);
    }

    // -- base image variant -------------------------------------------------

    #[test]
    fn base_alpine() {
        let cli = parse(&["-b", "alpine"]);
        assert_eq!(cli.base, Some(BaseImageVariant::Alpine));
    }

    #[test]
    fn base_slim() {
        let cli = parse(&["--base", "slim"]);
        assert_eq!(cli.base, Some(BaseImageVariant::Slim));
    }

    #[test]
    fn base_distroless() {
        let cli = parse(&["-b", "distroless"]);
        assert_eq!(cli.base, Some(BaseImageVariant::Distroless));
    }

    #[test]
    fn base_default() {
        let cli = parse(&["-b", "default"]);
        assert_eq!(cli.base, Some(BaseImageVariant::Default));
    }

    #[test]
    fn base_invalid() {
        let result = Cli::try_parse_from(["dockgen", "-b", "invalid"]);
        assert!(result.is_err());
    }

    // -- output dir ---------------------------------------------------------

    #[test]
    fn output_dir() {
        let cli = parse(&["-o", "/tmp/out"]);
        assert_eq!(cli.output_dir, Some(PathBuf::from("/tmp/out")));
    }

    // -- get_target_path ----------------------------------------------------

    #[test]
    fn target_path_absolute() {
        let cli = parse(&["/absolute/path"]);
        assert_eq!(cli.get_target_path(), PathBuf::from("/absolute/path"));
    }

    #[test]
    fn target_path_relative() {
        let cli = parse(&["relative/path"]);
        let resolved = cli.get_target_path();
        assert!(resolved.is_absolute());
        assert!(resolved.to_string_lossy().ends_with("relative/path"));
    }

    // -- generation config with overrides -----------------------------------

    #[test]
    fn generation_config_full() {
        let cli = parse(&[
            "-b",
            "alpine",
            "-p",
            "3000,8000",
            "--single",
            "--dry-run",
            "--compose",
            "-o",
            "/tmp/out",
        ]);
        let cfg = cli.to_generation_config();
        assert_eq!(cfg.base_image_override, Some(BaseImageVariant::Alpine));
        assert_eq!(cfg.port_overrides, vec![3000, 8000]);
        assert!(cfg.force_single);
        assert!(cfg.dry_run);
        assert!(cfg.emit_compose);
        assert_eq!(cfg.output_dir, Some(PathBuf::from("/tmp/out")));
    }

    // -- clap error cases ---------------------------------------------------

    #[test]
    fn unknown_flag() {
        let result = Cli::try_parse_from(["dockgen", "--nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn missing_port_value() {
        let result = Cli::try_parse_from(["dockgen", "-p"]);
        assert!(result.is_err());
    }

    #[test]
    fn missing_lang_value() {
        let result = Cli::try_parse_from(["dockgen", "-l"]);
        assert!(result.is_err());
    }
}
