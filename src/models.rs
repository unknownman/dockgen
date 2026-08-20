use std::fmt;
use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

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
    /// Human-readable name (typically the directory or package name).
    pub name: String,

    /// Absolute path to the service root on disk.
    pub path: PathBuf,

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
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: GenerationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, deserialized);
    }
}
