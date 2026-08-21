use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Directory names that are always excluded from traversal and heuristic scans.
pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".output",
    ".cache",
    "bin",
    "obj",
];

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

/// Detected programming language of a service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    NodeJs,
    Python,
    Go,
    Rust,
    Java,
    Php,
    DotNet,
    Ruby,
    Unknown(String),
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::NodeJs => write!(f, "Node.js"),
            Language::Python => write!(f, "Python"),
            Language::Go => write!(f, "Go"),
            Language::Rust => write!(f, "Rust"),
            Language::Java => write!(f, "Java"),
            Language::Php => write!(f, "PHP"),
            Language::DotNet => write!(f, ".NET"),
            Language::Ruby => write!(f, "Ruby"),
            Language::Unknown(name) => write!(f, "Unknown({name})"),
        }
    }
}

// ---------------------------------------------------------------------------
// PackageManager
// ---------------------------------------------------------------------------

/// Detected package / dependency manager for a service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Pip,
    Poetry,
    Pipenv,
    Cargo,
    GoModules,
    Maven,
    Gradle,
    Composer,
    Nuget,
    Bundler,
    Unknown,
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageManager::Npm => write!(f, "npm"),
            PackageManager::Pnpm => write!(f, "pnpm"),
            PackageManager::Yarn => write!(f, "yarn"),
            PackageManager::Bun => write!(f, "bun"),
            PackageManager::Pip => write!(f, "pip"),
            PackageManager::Poetry => write!(f, "poetry"),
            PackageManager::Pipenv => write!(f, "pipenv"),
            PackageManager::Cargo => write!(f, "cargo"),
            PackageManager::GoModules => write!(f, "go modules"),
            PackageManager::Maven => write!(f, "maven"),
            PackageManager::Gradle => write!(f, "gradle"),
            PackageManager::Composer => write!(f, "composer"),
            PackageManager::Nuget => write!(f, "nuget"),
            PackageManager::Bundler => write!(f, "bundler"),
            PackageManager::Unknown => write!(f, "unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Framework
// ---------------------------------------------------------------------------

/// Detected application framework for a service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Framework {
    // Node.js
    NextJs,
    Nuxt,
    NestJs,
    Express,
    Fastify,
    Remix,
    SvelteKit,
    Astro,
    NodeGeneric,

    // Python
    FastApi,
    Django,
    Flask,
    Starlette,
    Litestar,
    PythonGeneric,

    // Go
    Gin,
    Echo,
    Fiber,
    Chi,
    GoGeneric,

    // Rust
    ActixWeb,
    Axum,
    Rocket,
    Warp,
    RustGeneric,

    // Java
    SpringBoot,
    Quarkus,
    Micronaut,
    JavaGeneric,

    // PHP
    Laravel,
    Symfony,
    PhpGeneric,

    // .NET
    AspNetCore,
    DotNetGeneric,

    // Ruby
    Rails,
    Sinatra,
    RubyGeneric,

    // Fallback
    Generic,
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Framework::NextJs => "Next.js",
            Framework::Nuxt => "Nuxt",
            Framework::NestJs => "NestJS",
            Framework::Express => "Express",
            Framework::Fastify => "Fastify",
            Framework::Remix => "Remix",
            Framework::SvelteKit => "SvelteKit",
            Framework::Astro => "Astro",
            Framework::NodeGeneric => "Node.js (generic)",
            Framework::FastApi => "FastAPI",
            Framework::Django => "Django",
            Framework::Flask => "Flask",
            Framework::Starlette => "Starlette",
            Framework::Litestar => "Litestar",
            Framework::PythonGeneric => "Python (generic)",
            Framework::Gin => "Gin",
            Framework::Echo => "Echo",
            Framework::Fiber => "Fiber",
            Framework::Chi => "Chi",
            Framework::GoGeneric => "Go (generic)",
            Framework::ActixWeb => "Actix Web",
            Framework::Axum => "Axum",
            Framework::Rocket => "Rocket",
            Framework::Warp => "Warp",
            Framework::RustGeneric => "Rust (generic)",
            Framework::SpringBoot => "Spring Boot",
            Framework::Quarkus => "Quarkus",
            Framework::Micronaut => "Micronaut",
            Framework::JavaGeneric => "Java (generic)",
            Framework::Laravel => "Laravel",
            Framework::Symfony => "Symfony",
            Framework::PhpGeneric => "PHP (generic)",
            Framework::AspNetCore => "ASP.NET Core",
            Framework::DotNetGeneric => ".NET (generic)",
            Framework::Rails => "Rails",
            Framework::Sinatra => "Sinatra",
            Framework::RubyGeneric => "Ruby (generic)",
            Framework::Generic => "Generic",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// BaseImageVariant
// ---------------------------------------------------------------------------

/// Selects the base image variant for generated Dockerfiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
pub enum BaseImageVariant {
    Alpine,
    Slim,
    Distroless,
    Default,
}

impl fmt::Display for BaseImageVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseImageVariant::Alpine => write!(f, "alpine"),
            BaseImageVariant::Slim => write!(f, "slim"),
            BaseImageVariant::Distroless => write!(f, "distroless"),
            BaseImageVariant::Default => write!(f, "default"),
        }
    }
}

// ---------------------------------------------------------------------------
// InfraKind
// ---------------------------------------------------------------------------

/// Type of infrastructure service detected in a project.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ValueEnum,
)]
pub enum InfraKind {
    Postgres,
    Mysql,
    Redis,
    Mongo,
    RabbitMq,
    Kafka,
    Sqlite,
}

impl fmt::Display for InfraKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InfraKind::Postgres => write!(f, "PostgreSQL"),
            InfraKind::Mysql => write!(f, "MySQL"),
            InfraKind::Redis => write!(f, "Redis"),
            InfraKind::Mongo => write!(f, "MongoDB"),
            InfraKind::RabbitMq => write!(f, "RabbitMQ"),
            InfraKind::Kafka => write!(f, "Apache Kafka"),
            InfraKind::Sqlite => write!(f, "SQLite"),
        }
    }
}

#[allow(dead_code)]
impl InfraKind {
    /// Default port for this infrastructure type.
    pub fn default_port(&self) -> u16 {
        match self {
            InfraKind::Postgres => 5432,
            InfraKind::Mysql => 3306,
            InfraKind::Redis => 6379,
            InfraKind::Mongo => 27017,
            InfraKind::RabbitMq => 5672,
            InfraKind::Kafka => 9092,
            InfraKind::Sqlite => 0,
        }
    }

    /// Default Docker image tag for this infrastructure type.
    pub fn default_image(&self) -> &'static str {
        match self {
            InfraKind::Postgres => "postgres:16-alpine",
            InfraKind::Mysql => "mysql:8.0",
            InfraKind::Redis => "redis:7-alpine",
            InfraKind::Mongo => "mongo:7",
            InfraKind::RabbitMq => "rabbitmq:3-management-alpine",
            InfraKind::Kafka => "confluentinc/cp-kafka:latest",
            InfraKind::Sqlite => "alpine:3.20",
        }
    }

    /// Default environment variables for this infrastructure type.
    ///
    /// Returns `(key, value)` pairs with sensible non-secret defaults.
    pub fn default_env(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            InfraKind::Postgres => vec![
                ("POSTGRES_USER", "app"),
                ("POSTGRES_PASSWORD", "postgres"),
                ("POSTGRES_DB", "app"),
            ],
            InfraKind::Mysql => vec![
                ("MYSQL_ROOT_PASSWORD", "root"),
                ("MYSQL_DATABASE", "app"),
                ("MYSQL_USER", "app"),
                ("MYSQL_PASSWORD", "app"),
            ],
            InfraKind::Redis => vec![],
            InfraKind::Mongo => vec![
                ("MONGO_INITDB_ROOT_USERNAME", "app"),
                ("MONGO_INITDB_ROOT_PASSWORD", "app"),
            ],
            InfraKind::RabbitMq => vec![
                ("RABBITMQ_DEFAULT_USER", "guest"),
                ("RABBITMQ_DEFAULT_PASS", "guest"),
            ],
            InfraKind::Kafka => vec![],
            InfraKind::Sqlite => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// InfraSource
// ---------------------------------------------------------------------------

/// How an infrastructure dependency was detected.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum InfraSource {
    /// Detected via an environment variable (e.g. `DATABASE_URL`).
    EnvVar(String),
    /// Detected via a manifest dependency (e.g. `pg` in `package.json`).
    ManifestDependency(String),
    /// Detected via a configuration file (e.g. `prisma/schema.prisma`).
    ConfigFile(String),
    /// Detected via a Prisma schema with an explicit datasource provider.
    PrismaSchema,
    /// User explicitly provided via CLI flag or interactive prompt.
    ManualOverride,
}

impl fmt::Display for InfraSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InfraSource::EnvVar(var) => write!(f, "env var `{var}`"),
            InfraSource::ManifestDependency(dep) => write!(f, "dependency `{dep}`"),
            InfraSource::ConfigFile(path) => write!(f, "config file `{path}`"),
            InfraSource::PrismaSchema => write!(f, "Prisma schema"),
            InfraSource::ManualOverride => write!(f, "manual override"),
        }
    }
}

// ---------------------------------------------------------------------------
// InfraService
// ---------------------------------------------------------------------------

/// A detected or configured infrastructure service to include in
/// `docker-compose.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfraService {
    /// The type of infrastructure (Postgres, Redis, etc.).
    pub kind: InfraKind,

    /// Compose service name (e.g. `"postgres"`, `"redis"`).
    pub name: String,

    /// Docker image to use (e.g. `"postgres:16-alpine"`).
    pub image: String,

    /// Exposed port on the host.
    pub port: u16,

    /// Environment variables as `(key, value)` pairs, sorted by key.
    pub env_vars: Vec<(String, String)>,

    /// Whether this service should be included in the generated compose file.
    pub is_attached_to_compose: bool,

    /// How this infrastructure dependency was detected.
    pub source: InfraSource,
}

// ---------------------------------------------------------------------------
// InteractiveAnswers
// ---------------------------------------------------------------------------

/// Answers collected from the Phase 2 interactive Q&A wizard.
///
/// All fields are optional or default so that non-interactive mode (`-y` /
/// `--json`) can skip the wizard entirely while still producing valid output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveAnswers {
    /// Which infrastructure kinds the user wants included in compose.
    pub include_infra_in_compose: Vec<InfraKind>,

    /// Whether Prisma migrations should run on startup.
    pub run_prisma_migrations: Option<bool>,

    /// Whether a Laravel queue worker service should be generated.
    pub create_queue_worker: Option<bool>,

    /// User-specified port overrides keyed by compose service name.
    pub custom_service_ports: BTreeMap<String, u16>,
}

// ---------------------------------------------------------------------------
// ServiceType
// ---------------------------------------------------------------------------

/// The role a service plays within the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceType {
    Single,
    Frontend,
    Backend,
    Worker,
    Api,
    MonorepoMember,
}

impl fmt::Display for ServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceType::Single => write!(f, "single"),
            ServiceType::Frontend => write!(f, "frontend"),
            ServiceType::Backend => write!(f, "backend"),
            ServiceType::Worker => write!(f, "worker"),
            ServiceType::Api => write!(f, "api"),
            ServiceType::MonorepoMember => write!(f, "monorepo-member"),
        }
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// A single deployable unit detected within the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    /// Human-readable name (typically the directory name).
    pub name: String,

    /// Absolute path to the service root on disk.
    pub path: PathBuf,

    /// Package / binary name extracted from manifests (e.g. from `Cargo.toml`
    /// `[package] name`, `package.json` `"name"`, `<artifactId>`, etc.).
    /// Falls back to `None` when no manifest is available.
    pub package_name: Option<String>,

    /// Primary programming language.
    pub language: Language,

    /// Detected framework (or generic fallback).
    pub framework: Framework,

    /// Detected package / dependency manager.
    pub package_manager: PackageManager,

    /// Runtime version constraint (e.g. `"20"`, `"3.12"`, `"1.78"`).
    pub runtime_version: Option<String>,

    /// Custom entrypoint command, if any.
    pub entrypoint: Option<String>,

    /// TCP ports the service exposes.
    pub exposed_ports: Vec<u16>,

    /// Environment variables required at runtime (key, value) pairs.
    pub env_vars: Vec<(String, String)>,

    /// The role this service plays.
    pub service_type: ServiceType,

    /// Custom build command override.
    pub build_command: Option<String>,

    /// Custom start / run command override.
    pub start_command: Option<String>,

    /// Whether this service is part of a monorepo workspace.
    pub is_monorepo: bool,
}

// ---------------------------------------------------------------------------
// ProjectAnalysis
// ---------------------------------------------------------------------------

/// Result of scanning and analysing a project directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAnalysis {
    /// Absolute path to the project root.
    pub root_path: PathBuf,

    /// Whether the project is a monorepo with multiple services.
    pub is_monorepo: bool,

    /// Workspace orchestration tool, if detected (e.g. `"turborepo"`, `"pnpm"`, `"cargo"`).
    pub workspace_tool: Option<String>,

    /// All detected services, sorted by name for deterministic output.
    pub services: Vec<Service>,

    /// Infrastructure services detected in the project (env vars, manifests,
    /// config files), sorted by kind for deterministic output.
    pub detected_infrastructures: Vec<InfraService>,

    /// Non-fatal warnings encountered during analysis.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// GenerationConfig
// ---------------------------------------------------------------------------

/// User-supplied overrides that influence Dockerfile/dockerignore generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Override the auto-detected base image variant for all services.
    pub base_image_override: Option<BaseImageVariant>,

    /// Override exposed ports for all services (replaces detected ports).
    pub port_overrides: Vec<u16>,

    /// Force single-service output even when a monorepo is detected.
    pub force_single: bool,

    /// Print generated files to stdout instead of writing to disk.
    pub dry_run: bool,

    /// Also emit a `docker-compose.yml` alongside individual Dockerfiles.
    pub emit_compose: bool,

    /// Output directory override. Defaults to each service's root path.
    pub output_dir: Option<PathBuf>,

    /// Enable the Phase 2 interactive Q&A wizard.
    pub interactive: bool,

    /// Accept all interactive defaults without prompting (implies `--yes`).
    pub assume_yes: bool,

    /// User answers from the interactive wizard, if any.
    pub interactive_answers: Option<InteractiveAnswers>,
}

// ---------------------------------------------------------------------------
// GeneratedFile
// ---------------------------------------------------------------------------

/// A single file produced by the generation pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedFile {
    /// Path relative to the output root (e.g. `services/api/Dockerfile`).
    pub relative_path: PathBuf,

    /// Full textual content of the generated file.
    pub content: String,

    /// Human-readable description of what this file contains.
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_display() {
        assert_eq!(Language::NodeJs.to_string(), "Node.js");
        assert_eq!(Language::DotNet.to_string(), ".NET");
        assert_eq!(Language::Unknown("Zig".into()).to_string(), "Unknown(Zig)");
    }

    #[test]
    fn framework_display() {
        assert_eq!(Framework::NextJs.to_string(), "Next.js");
        assert_eq!(Framework::ActixWeb.to_string(), "Actix Web");
        assert_eq!(Framework::AspNetCore.to_string(), "ASP.NET Core");
        assert_eq!(Framework::Generic.to_string(), "Generic");
    }

    #[test]
    fn package_manager_display() {
        assert_eq!(PackageManager::Npm.to_string(), "npm");
        assert_eq!(PackageManager::GoModules.to_string(), "go modules");
        assert_eq!(PackageManager::Unknown.to_string(), "unknown");
    }

    #[test]
    fn base_image_variant_display() {
        assert_eq!(BaseImageVariant::Alpine.to_string(), "alpine");
        assert_eq!(BaseImageVariant::Slim.to_string(), "slim");
        assert_eq!(BaseImageVariant::Distroless.to_string(), "distroless");
        assert_eq!(BaseImageVariant::Default.to_string(), "default");
    }

    #[test]
    fn service_type_display() {
        assert_eq!(ServiceType::Single.to_string(), "single");
        assert_eq!(ServiceType::MonorepoMember.to_string(), "monorepo-member");
    }

    #[test]
    fn service_serde_roundtrip() {
        let svc = Service {
            name: "api".into(),
            path: PathBuf::from("/project/services/api"),
            package_name: Some("my-api".into()),
            language: Language::Rust,
            framework: Framework::Axum,
            package_manager: PackageManager::Cargo,
            runtime_version: Some("1.78.0".into()),
            entrypoint: None,
            exposed_ports: vec![8080],
            env_vars: vec![("RUST_LOG".into(), "info".into())],
            service_type: ServiceType::Api,
            build_command: None,
            start_command: None,
            is_monorepo: true,
        };

        let json = serde_json::to_string(&svc).unwrap();
        let deserialized: Service = serde_json::from_str(&json).unwrap();
        assert_eq!(svc, deserialized);
    }

    #[test]
    fn project_analysis_serde_roundtrip() {
        let analysis = ProjectAnalysis {
            root_path: PathBuf::from("/project"),
            is_monorepo: true,
            workspace_tool: Some("turborepo".into()),
            services: vec![],
            detected_infrastructures: vec![],
            warnings: vec!["No lockfile found".into()],
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: ProjectAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(analysis, deserialized);
    }

    #[test]
    fn generated_file_serde_roundtrip() {
        let gf = GeneratedFile {
            relative_path: PathBuf::from("services/web/Dockerfile"),
            content: "FROM node:20-alpine\n".into(),
            description: "Dockerfile for web service".into(),
        };

        let json = serde_json::to_string(&gf).unwrap();
        let deserialized: GeneratedFile = serde_json::from_str(&json).unwrap();
        assert_eq!(gf, deserialized);
    }

    #[test]
    fn generation_config_defaults_via_deserialize() {
        let cfg = GenerationConfig {
            base_image_override: None,
            port_overrides: vec![],
            force_single: false,
            dry_run: false,
            emit_compose: false,
            output_dir: None,
            interactive: false,
            assume_yes: false,
            interactive_answers: None,
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: GenerationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn infra_kind_display() {
        assert_eq!(InfraKind::Postgres.to_string(), "PostgreSQL");
        assert_eq!(InfraKind::Mysql.to_string(), "MySQL");
        assert_eq!(InfraKind::Redis.to_string(), "Redis");
        assert_eq!(InfraKind::Mongo.to_string(), "MongoDB");
        assert_eq!(InfraKind::RabbitMq.to_string(), "RabbitMQ");
        assert_eq!(InfraKind::Kafka.to_string(), "Apache Kafka");
        assert_eq!(InfraKind::Sqlite.to_string(), "SQLite");
    }

    #[test]
    fn infra_kind_default_port() {
        assert_eq!(InfraKind::Postgres.default_port(), 5432);
        assert_eq!(InfraKind::Mysql.default_port(), 3306);
        assert_eq!(InfraKind::Redis.default_port(), 6379);
        assert_eq!(InfraKind::Mongo.default_port(), 27017);
        assert_eq!(InfraKind::RabbitMq.default_port(), 5672);
        assert_eq!(InfraKind::Kafka.default_port(), 9092);
        assert_eq!(InfraKind::Sqlite.default_port(), 0);
    }

    #[test]
    fn infra_kind_default_image() {
        assert_eq!(InfraKind::Postgres.default_image(), "postgres:16-alpine");
        assert_eq!(InfraKind::Mysql.default_image(), "mysql:8.0");
        assert_eq!(InfraKind::Redis.default_image(), "redis:7-alpine");
        assert_eq!(InfraKind::Mongo.default_image(), "mongo:7");
        assert_eq!(
            InfraKind::RabbitMq.default_image(),
            "rabbitmq:3-management-alpine"
        );
        assert_eq!(
            InfraKind::Kafka.default_image(),
            "confluentinc/cp-kafka:latest"
        );
        assert_eq!(InfraKind::Sqlite.default_image(), "alpine:3.20");
    }

    #[test]
    fn infra_kind_default_env() {
        let pg_env = InfraKind::Postgres.default_env();
        assert!(pg_env.iter().any(|(k, _)| *k == "POSTGRES_USER"));
        assert!(pg_env
            .iter()
            .any(|(k, v)| *k == "POSTGRES_DB" && *v == "app"));

        let mysql_env = InfraKind::Mysql.default_env();
        assert!(mysql_env.iter().any(|(k, _)| *k == "MYSQL_ROOT_PASSWORD"));

        // Redis and Kafka have no default env vars.
        assert!(InfraKind::Redis.default_env().is_empty());
        assert!(InfraKind::Kafka.default_env().is_empty());
    }

    #[test]
    fn infra_kind_ordering() {
        // InfraKind derives Ord — sorted by variant declaration order:
        // Postgres(0), Mysql(1), Redis(2), Mongo(3), RabbitMq(4), Kafka(5), Sqlite(6)
        let mut kinds = vec![InfraKind::Redis, InfraKind::Postgres, InfraKind::Mysql];
        kinds.sort();
        assert_eq!(
            kinds,
            vec![InfraKind::Postgres, InfraKind::Mysql, InfraKind::Redis]
        );
    }

    #[test]
    fn infra_source_display() {
        assert_eq!(
            InfraSource::EnvVar("DATABASE_URL".into()).to_string(),
            "env var `DATABASE_URL`"
        );
        assert_eq!(
            InfraSource::ManifestDependency("pg".into()).to_string(),
            "dependency `pg`"
        );
        assert_eq!(
            InfraSource::ConfigFile("prisma/schema.prisma".into()).to_string(),
            "config file `prisma/schema.prisma`"
        );
        assert_eq!(InfraSource::PrismaSchema.to_string(), "Prisma schema");
        assert_eq!(InfraSource::ManualOverride.to_string(), "manual override");
    }

    #[test]
    fn infra_source_ordering() {
        // InfraSource derives Ord — sorted by variant declaration order:
        // EnvVar(0), ManifestDependency(1), ConfigFile(2), PrismaSchema(3), ManualOverride(4)
        let mut sources = vec![
            InfraSource::ManualOverride,
            InfraSource::EnvVar("A".into()),
            InfraSource::PrismaSchema,
        ];
        sources.sort();
        assert_eq!(
            sources,
            vec![
                InfraSource::EnvVar("A".into()),
                InfraSource::PrismaSchema,
                InfraSource::ManualOverride,
            ]
        );
    }

    #[test]
    fn infra_service_serde_roundtrip() {
        let svc = InfraService {
            kind: InfraKind::Postgres,
            name: "postgres".into(),
            image: "postgres:16-alpine".into(),
            port: 5432,
            env_vars: vec![
                ("POSTGRES_DB".into(), "app".into()),
                ("POSTGRES_PASSWORD".into(), "postgres".into()),
            ],
            is_attached_to_compose: true,
            source: InfraSource::EnvVar("DATABASE_URL".into()),
        };

        let json = serde_json::to_string(&svc).unwrap();
        let deserialized: InfraService = serde_json::from_str(&json).unwrap();
        assert_eq!(svc, deserialized);
    }

    #[test]
    fn interactive_answers_default() {
        let answers = InteractiveAnswers::default();
        assert!(answers.include_infra_in_compose.is_empty());
        assert!(answers.run_prisma_migrations.is_none());
        assert!(answers.create_queue_worker.is_none());
        assert!(answers.custom_service_ports.is_empty());
    }

    #[test]
    fn interactive_answers_serde_roundtrip() {
        let answers = InteractiveAnswers {
            include_infra_in_compose: vec![InfraKind::Postgres, InfraKind::Redis],
            run_prisma_migrations: Some(true),
            create_queue_worker: Some(false),
            custom_service_ports: BTreeMap::from([
                ("postgres".into(), 5433),
                ("redis".into(), 6380),
            ]),
        };

        let json = serde_json::to_string(&answers).unwrap();
        let deserialized: InteractiveAnswers = serde_json::from_str(&json).unwrap();
        assert_eq!(answers, deserialized);
    }
}
