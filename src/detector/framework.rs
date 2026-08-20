use std::path::Path;

use crate::analyzer::dependencies::ManifestInfo;
use crate::models::{Framework, Language};

// ---------------------------------------------------------------------------
// FrameworkDetectionResult
// ---------------------------------------------------------------------------

/// Result of framework detection for a single service directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkDetectionResult {
    /// Detected framework (or generic fallback).
    pub framework: Framework,
    /// Default container port for this framework.
    pub default_port: u16,
    /// Recommended build command, if any.
    pub default_build_cmd: Option<String>,
    /// Recommended start command, if any.
    pub default_start_cmd: Option<String>,
    /// Recommended environment variables.
    pub env_vars: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detects the framework for a service at `dir_path` using its [`ManifestInfo`]
/// and the detected [`Language`].
///
/// Dispatches to the appropriate ecosystem detector based on `language`. When
/// the language is [`Language::Unknown`], all ecosystem detectors are tried as
/// a fallback.
pub fn detect_framework(
    dir_path: &Path,
    manifest: &ManifestInfo,
    language: &Language,
) -> FrameworkDetectionResult {
    let deps = &manifest.dependencies;
    let raw = &manifest.raw_content;

    let result = match language {
        Language::NodeJs => detect_node_framework(dir_path, deps, raw),
        Language::Python => detect_python_framework(dir_path, deps),
        Language::Go => detect_go_framework(deps),
        Language::Rust => detect_rust_framework(deps),
        Language::Java => detect_java_framework(dir_path, deps, raw),
        Language::Php => detect_php_framework(dir_path, deps),
        Language::DotNet => detect_dotnet_framework(raw),
        Language::Ruby => detect_ruby_framework(dir_path, deps),
        Language::Unknown(_) => {
            // Try all detectors as fallback for unknown languages.
            // Each branch is guarded by an ecosystem indicator check so we
            // don't falsely claim a framework for an unrelated project.
            if has_node_indicators(deps, raw) {
                if let Some(r) = detect_node_framework(dir_path, deps, raw) {
                    return r;
                }
            }
            if has_python_indicators(dir_path) || has_python_deps(deps) {
                if let Some(r) = detect_python_framework(dir_path, deps) {
                    return r;
                }
            }
            if has_go_indicators(deps) {
                if let Some(r) = detect_go_framework(deps) {
                    return r;
                }
            }
            if raw.contains_key("Cargo.toml") || dir_path.join("Cargo.toml").is_file() {
                if let Some(r) = detect_rust_framework(deps) {
                    return r;
                }
            }
            if has_java_indicators(dir_path, raw) {
                if let Some(r) = detect_java_framework(dir_path, deps, raw) {
                    return r;
                }
            }
            if has_php_indicators(dir_path, deps) {
                if let Some(r) = detect_php_framework(dir_path, deps) {
                    return r;
                }
            }
            if has_dotnet_indicators(raw) {
                if let Some(r) = detect_dotnet_framework(raw) {
                    return r;
                }
            }
            if has_ruby_indicators(dir_path, deps) {
                if let Some(r) = detect_ruby_framework(dir_path, deps) {
                    return r;
                }
            }
            None
        }
    };

    result.unwrap_or_else(|| FrameworkDetectionResult {
        framework: Framework::Generic,
        default_port: 8080,
        default_build_cmd: None,
        default_start_cmd: None,
        env_vars: vec![],
    })
}

// ---------------------------------------------------------------------------
// Node.js detection
// ---------------------------------------------------------------------------

fn detect_node_framework(
    dir: &Path,
    deps: &[String],
    _raw: &std::collections::HashMap<String, String>,
) -> Option<FrameworkDetectionResult> {
    // Next.js
    if deps.iter().any(|d| d == "next")
        || file_exists_any(
            dir,
            &["next.config.js", "next.config.mjs", "next.config.ts"],
        )
    {
        return Some(node_result(
            Framework::NextJs,
            3000,
            Some("next build".into()),
            Some("next start".into()),
        ));
    }

    // Nuxt
    if deps.iter().any(|d| d == "nuxt" || d == "nuxt3")
        || file_exists_any(dir, &["nuxt.config.ts", "nuxt.config.js"])
    {
        return Some(node_result(
            Framework::Nuxt,
            3000,
            Some("nuxt build".into()),
            Some("node .output/server/index.mjs".into()),
        ));
    }

    // NestJS
    if deps
        .iter()
        .any(|d| d == "@nestjs/core" || d == "@nestjs/common")
        || dir.join("nest-cli.json").is_file()
    {
        return Some(node_result(
            Framework::NestJs,
            3000,
            Some("nest build".into()),
            Some("node dist/main".into()),
        ));
    }

    // Remix
    if deps
        .iter()
        .any(|d| d == "@remix-run/node" || d == "@remix-run/react")
        || dir.join("remix.config.js").is_file()
    {
        return Some(node_result(
            Framework::Remix,
            3000,
            Some("remix build".into()),
            Some("remix-serve ./build/index.js".into()),
        ));
    }

    // SvelteKit
    if deps.iter().any(|d| d == "@sveltejs/kit") || dir.join("svelte.config.js").is_file() {
        return Some(node_result(
            Framework::SvelteKit,
            3000,
            Some("vite build".into()),
            Some("vite preview".into()),
        ));
    }

    // Astro
    if deps.iter().any(|d| d == "astro") || dir.join("astro.config.mjs").is_file() {
        return Some(node_result(
            Framework::Astro,
            4321,
            Some("astro build".into()),
            Some("astro preview".into()),
        ));
    }

    // Fastify
    if deps.iter().any(|d| d == "fastify") {
        return Some(node_result(
            Framework::Fastify,
            3000,
            None,
            Some("node server.js".into()),
        ));
    }

    // Express
    if deps.iter().any(|d| d == "express") {
        return Some(node_result(
            Framework::Express,
            3000,
            None,
            Some("node app.js".into()),
        ));
    }

    // Node.js generic fallback.
    Some(node_result(
        Framework::NodeGeneric,
        3000,
        None,
        Some("node index.js".into()),
    ))
}

fn node_result(
    fw: Framework,
    port: u16,
    build: Option<String>,
    start: Option<String>,
) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: port,
        default_build_cmd: build,
        default_start_cmd: start,
        env_vars: vec![("NODE_ENV".into(), "production".into())],
    }
}

fn has_node_indicators(deps: &[String], raw: &std::collections::HashMap<String, String>) -> bool {
    // If we have a package.json in raw_content, it's Node.js.
    if raw.contains_key("package.json") {
        return true;
    }
    // Common Node.js dependency names and prefixes.
    let node_dep_markers = [
        "next",
        "react",
        "vue",
        "svelte",
        "angular",
        "express",
        "fastify",
        "koa",
        "hapi",
        "nuxt",
        "@nuxt",
        "nestjs",
        "@nestjs",
        "remix",
        "@remix-run",
        "astro",
        "@sveltejs",
        "tailwindcss",
        "vite",
        "webpack",
        "typescript",
        "eslint",
        "prettier",
        "@mui/",
        "@chakra-ui",
        "@radix-ui",
    ];
    deps.iter()
        .any(|d| node_dep_markers.iter().any(|m| d.contains(m)))
}

fn has_python_deps(deps: &[String]) -> bool {
    deps.iter().any(|d| {
        [
            "fastapi",
            "django",
            "flask",
            "starlette",
            "litestar",
            "starlite",
            "uvicorn",
            "gunicorn",
        ]
        .iter()
        .any(|m| d.contains(m))
    })
}

fn has_python_indicators(dir: &Path) -> bool {
    dir.join("pyproject.toml").is_file()
        || dir.join("requirements.txt").is_file()
        || dir.join("Pipfile").is_file()
        || dir.join("setup.py").is_file()
        || dir.join("manage.py").is_file()
}

fn has_go_indicators(deps: &[String]) -> bool {
    deps.iter()
        .any(|d| d.starts_with("github.com/") || d.starts_with("golang.org/"))
}

fn has_java_indicators(dir: &Path, raw: &std::collections::HashMap<String, String>) -> bool {
    raw.contains_key("pom.xml")
        || raw.contains_key("build.gradle")
        || raw.contains_key("build.gradle.kts")
        || dir.join("pom.xml").is_file()
        || dir.join("build.gradle").is_file()
        || dir.join("build.gradle.kts").is_file()
        || dir.join("gradlew").is_file()
}

fn has_php_indicators(dir: &Path, deps: &[String]) -> bool {
    deps.iter()
        .any(|d| d.contains("laravel") || d.contains("symfony"))
        || dir.join("composer.json").is_file()
        || dir.join("artisan").is_file()
        || dir.join("bin/console").is_file()
}

fn has_dotnet_indicators(raw: &std::collections::HashMap<String, String>) -> bool {
    raw.values()
        .any(|c| c.contains("Microsoft.NET.Sdk") || c.contains("Microsoft.AspNetCore"))
        || raw
            .keys()
            .any(|k| k.ends_with(".csproj") || k.ends_with(".fsproj"))
}

fn has_ruby_indicators(dir: &Path, deps: &[String]) -> bool {
    deps.iter().any(|d| {
        ["rails", "sinatra", "rack", "puma", "sidekiq"]
            .iter()
            .any(|m| d.contains(m))
    }) || dir.join("Gemfile").is_file()
        || dir.join("bin/rails").is_file()
        || dir.join("config/application.rb").is_file()
}

// ---------------------------------------------------------------------------
// Python detection
// ---------------------------------------------------------------------------

fn detect_python_framework(dir: &Path, deps: &[String]) -> Option<FrameworkDetectionResult> {
    // FastAPI
    if deps.iter().any(|d| d == "fastapi") {
        return Some(py_result(
            Framework::FastApi,
            8000,
            Some("uvicorn main:app --host 0.0.0.0".into()),
        ));
    }

    // Django
    if deps.iter().any(|d| d == "django") || dir.join("manage.py").is_file() {
        return Some(py_result(
            Framework::Django,
            8000,
            Some("python manage.py migrate".into()),
        ));
    }

    // Flask
    if deps.iter().any(|d| d == "flask") {
        return Some(py_result(
            Framework::Flask,
            5000,
            Some("flask run --host 0.0.0.0".into()),
        ));
    }

    // Starlette
    if deps.iter().any(|d| d == "starlette") {
        return Some(py_result(Framework::Starlette, 8000, None));
    }

    // Litestar
    if deps.iter().any(|d| d == "litestar" || d == "starlite") {
        return Some(py_result(Framework::Litestar, 8000, None));
    }

    Some(py_result(Framework::PythonGeneric, 8000, None))
}

fn py_result(fw: Framework, port: u16, start: Option<String>) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: port,
        default_build_cmd: None,
        default_start_cmd: start,
        env_vars: vec![("PYTHONUNBUFFERED".into(), "1".into())],
    }
}

// ---------------------------------------------------------------------------
// Go detection
// ---------------------------------------------------------------------------

fn detect_go_framework(deps: &[String]) -> Option<FrameworkDetectionResult> {
    if deps.iter().any(|d| d.contains("gin-gonic/gin")) {
        return Some(go_result(Framework::Gin));
    }
    if deps.iter().any(|d| d.contains("labstack/echo")) {
        return Some(go_result(Framework::Echo));
    }
    if deps.iter().any(|d| d.contains("gofiber/fiber")) {
        return Some(go_result(Framework::Fiber));
    }
    if deps.iter().any(|d| d.contains("go-chi/chi")) {
        return Some(go_result(Framework::Chi));
    }

    Some(go_result(Framework::GoGeneric))
}

fn go_result(fw: Framework) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: 8080,
        default_build_cmd: Some("go build -o /app ./cmd/server".into()),
        default_start_cmd: Some("./server".into()),
        env_vars: vec![("CGO_ENABLED".into(), "0".into())],
    }
}

// ---------------------------------------------------------------------------
// Rust detection
// ---------------------------------------------------------------------------

fn detect_rust_framework(deps: &[String]) -> Option<FrameworkDetectionResult> {
    if deps.iter().any(|d| d == "axum") {
        return Some(rust_result(Framework::Axum, 8080));
    }
    if deps.iter().any(|d| d == "actix-web") {
        return Some(rust_result(Framework::ActixWeb, 8080));
    }
    if deps.iter().any(|d| d == "rocket") {
        return Some(rust_result(Framework::Rocket, 8000));
    }
    if deps.iter().any(|d| d == "warp") {
        return Some(rust_result(Framework::Warp, 8080));
    }

    Some(rust_result(Framework::RustGeneric, 8080))
}

fn rust_result(fw: Framework, port: u16) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: port,
        default_build_cmd: Some("cargo build --release".into()),
        default_start_cmd: Some("./target/release/app".into()),
        env_vars: vec![("RUST_LOG".into(), "info".into())],
    }
}

// ---------------------------------------------------------------------------
// Java detection
// ---------------------------------------------------------------------------

fn detect_java_framework(
    dir: &Path,
    deps: &[String],
    raw: &std::collections::HashMap<String, String>,
) -> Option<FrameworkDetectionResult> {
    // Read build file content from disk for content-based matching.
    let pom_content = raw.get("pom.xml").cloned().or_else(|| {
        if dir.join("pom.xml").is_file() {
            std::fs::read_to_string(dir.join("pom.xml")).ok()
        } else {
            None
        }
    });

    // Spring Boot
    if deps
        .iter()
        .any(|d| d.contains("spring-boot") || d.contains("org.springframework.boot"))
    {
        return Some(java_result(Framework::SpringBoot));
    }
    if pom_content
        .as_deref()
        .is_some_and(|c| c.contains("spring-boot"))
    {
        return Some(java_result(Framework::SpringBoot));
    }

    // Quarkus
    if deps
        .iter()
        .any(|d| d.contains("quarkus") || d.contains("io.quarkus"))
    {
        return Some(java_result(Framework::Quarkus));
    }
    if pom_content
        .as_deref()
        .is_some_and(|c| c.contains("quarkus"))
    {
        return Some(java_result(Framework::Quarkus));
    }

    // Micronaut
    if deps
        .iter()
        .any(|d| d.contains("micronaut") || d.contains("io.micronaut"))
    {
        return Some(java_result(Framework::Micronaut));
    }
    if pom_content
        .as_deref()
        .is_some_and(|c| c.contains("micronaut"))
    {
        return Some(java_result(Framework::Micronaut));
    }

    // If we have Java manifests but no specific framework, generic.
    Some(java_result(Framework::JavaGeneric))
}

fn java_result(fw: Framework) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: 8080,
        default_build_cmd: None, // Let the build tool handle it.
        default_start_cmd: None,
        env_vars: vec![("JAVA_OPTS".into(), "-Xmx512m".into())],
    }
}

// ---------------------------------------------------------------------------
// PHP detection
// ---------------------------------------------------------------------------

fn detect_php_framework(dir: &Path, deps: &[String]) -> Option<FrameworkDetectionResult> {
    // Laravel
    if deps.iter().any(|d| d.contains("laravel/framework")) || dir.join("artisan").is_file() {
        return Some(php_result(
            Framework::Laravel,
            8000,
            Some("php artisan optimize".into()),
            Some("php artisan serve --host=0.0.0.0".into()),
        ));
    }

    // Symfony
    if deps.iter().any(|d| d.contains("symfony/framework-bundle"))
        || dir.join("bin/console").is_file()
    {
        return Some(php_result(
            Framework::Symfony,
            8000,
            Some("symfony build".into()),
            Some("symfony server:start".into()),
        ));
    }

    Some(php_result(
        Framework::PhpGeneric,
        80,
        None,
        Some("php -S 0.0.0.0:80 -t public".into()),
    ))
}

fn php_result(
    fw: Framework,
    port: u16,
    build: Option<String>,
    start: Option<String>,
) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: port,
        default_build_cmd: build,
        default_start_cmd: start,
        env_vars: vec![("APP_ENV".into(), "production".into())],
    }
}

// ---------------------------------------------------------------------------
// .NET detection
// ---------------------------------------------------------------------------

fn detect_dotnet_framework(
    raw: &std::collections::HashMap<String, String>,
) -> Option<FrameworkDetectionResult> {
    // Check csproj files in raw_content for ASP.NET Core SDK.
    let has_aspnet = raw.values().any(|content| {
        content.contains("Microsoft.NET.Sdk.Web") || content.contains("Microsoft.AspNetCore")
    });

    if !has_aspnet && !raw.contains_key("*.csproj") {
        return None;
    }

    Some(dotnet_result(Framework::AspNetCore))
}

fn dotnet_result(fw: Framework) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: 8080,
        default_build_cmd: Some("dotnet publish -c Release -o /app".into()),
        default_start_cmd: Some("dotnet app.dll".into()),
        env_vars: vec![("ASPNETCORE_URLS".into(), "http://+:8080".into())],
    }
}

// ---------------------------------------------------------------------------
// Ruby detection
// ---------------------------------------------------------------------------

fn detect_ruby_framework(dir: &Path, deps: &[String]) -> Option<FrameworkDetectionResult> {
    // Rails
    if deps.iter().any(|d| d == "rails")
        || dir.join("bin/rails").is_file()
        || dir.join("config/application.rb").is_file()
    {
        return Some(ruby_result(
            Framework::Rails,
            3000,
            Some("bundle exec rails assets:precompile".into()),
            Some("bundle exec rails server -b 0.0.0.0".into()),
        ));
    }

    // Sinatra
    if deps.iter().any(|d| d == "sinatra") {
        return Some(ruby_result(
            Framework::Sinatra,
            4567,
            None,
            Some("ruby app.rb".into()),
        ));
    }

    Some(ruby_result(
        Framework::RubyGeneric,
        3000,
        None,
        Some("ruby app.rb".into()),
    ))
}

fn ruby_result(
    fw: Framework,
    port: u16,
    build: Option<String>,
    start: Option<String>,
) -> FrameworkDetectionResult {
    FrameworkDetectionResult {
        framework: fw,
        default_port: port,
        default_build_cmd: build,
        default_start_cmd: start,
        env_vars: vec![("RAILS_ENV".into(), "production".into())],
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Returns `true` if any of the given filenames exist in `dir`.
fn file_exists_any(dir: &Path, names: &[&str]) -> bool {
    names.iter().any(|n| dir.join(n).is_file())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        TempDir::new().unwrap()
    }

    fn manifest_with(deps: Vec<&str>) -> ManifestInfo {
        ManifestInfo {
            dependencies: deps.into_iter().map(String::from).collect(),
            ..ManifestInfo::default()
        }
    }

    fn manifest_with_raw(raw_entries: Vec<(&str, &str)>) -> ManifestInfo {
        ManifestInfo {
            raw_content: raw_entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..ManifestInfo::default()
        }
    }

    // -----------------------------------------------------------------------
    // Node.js
    // -----------------------------------------------------------------------

    #[test]
    fn node_nextjs_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["next", "react"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::NextJs);
        assert_eq!(r.default_port, 3000);
    }

    #[test]
    fn node_nextjs_by_config() {
        let d = tmp();
        fs::write(d.path().join("next.config.js"), "module.exports = {}").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::NextJs);
    }

    #[test]
    fn node_nextjs_by_config_mjs() {
        let d = tmp();
        fs::write(d.path().join("next.config.mjs"), "export default {}").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::NextJs);
    }

    #[test]
    fn node_nextjs_by_config_ts() {
        let d = tmp();
        fs::write(d.path().join("next.config.ts"), "export default {}").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::NextJs);
    }

    #[test]
    fn node_nuxt_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["nuxt"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Nuxt);
    }

    #[test]
    fn node_nuxt3_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["nuxt3"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Nuxt);
    }

    #[test]
    fn node_nuxt_by_config() {
        let d = tmp();
        fs::write(d.path().join("nuxt.config.ts"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::Nuxt);
    }

    #[test]
    fn node_nestjs_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["@nestjs/core"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::NestJs);
        assert_eq!(r.default_port, 3000);
    }

    #[test]
    fn node_nestjs_by_file() {
        let d = tmp();
        fs::write(d.path().join("nest-cli.json"), "{}").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::NestJs);
    }

    #[test]
    fn node_remix_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["@remix-run/node"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Remix);
    }

    #[test]
    fn node_remix_by_file() {
        let d = tmp();
        fs::write(d.path().join("remix.config.js"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::Remix);
    }

    #[test]
    fn node_sveltekit_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["@sveltejs/kit"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::SvelteKit);
    }

    #[test]
    fn node_sveltekit_by_file() {
        let d = tmp();
        fs::write(d.path().join("svelte.config.js"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::SvelteKit);
    }

    #[test]
    fn node_astro_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["astro"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Astro);
        assert_eq!(r.default_port, 4321);
    }

    #[test]
    fn node_astro_by_file() {
        let d = tmp();
        fs::write(d.path().join("astro.config.mjs"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::NodeJs);
        assert_eq!(r.framework, Framework::Astro);
    }

    #[test]
    fn node_fastify() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["fastify"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Fastify);
    }

    #[test]
    fn node_express() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["express"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::Express);
    }

    #[test]
    fn node_generic_fallback() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["lodash"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::NodeGeneric);
        assert_eq!(r.default_port, 3000);
    }

    #[test]
    fn node_env_var() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["express"]),
            &Language::NodeJs,
        );
        assert!(r
            .env_vars
            .iter()
            .any(|(k, v)| k == "NODE_ENV" && v == "production"));
    }

    // -----------------------------------------------------------------------
    // Python
    // -----------------------------------------------------------------------

    #[test]
    fn python_fastapi() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["fastapi", "uvicorn"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::FastApi);
        assert_eq!(r.default_port, 8000);
    }

    #[test]
    fn python_django_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["django"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::Django);
    }

    #[test]
    fn python_django_by_manage_py() {
        let d = tmp();
        fs::write(d.path().join("manage.py"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Python);
        assert_eq!(r.framework, Framework::Django);
    }

    #[test]
    fn python_flask() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["flask"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::Flask);
        assert_eq!(r.default_port, 5000);
    }

    #[test]
    fn python_starlette() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["starlette"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::Starlette);
    }

    #[test]
    fn python_litestar() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["litestar"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::Litestar);
    }

    #[test]
    fn python_starlite_alias() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["starlite"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::Litestar);
    }

    #[test]
    fn python_generic_fallback() {
        let d = tmp();
        fs::write(d.path().join("requirements.txt"), "requests\n").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Python);
        assert_eq!(r.framework, Framework::PythonGeneric);
    }

    #[test]
    fn python_env_var() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["fastapi"]),
            &Language::Python,
        );
        assert!(r
            .env_vars
            .iter()
            .any(|(k, v)| k == "PYTHONUNBUFFERED" && v == "1"));
    }

    // -----------------------------------------------------------------------
    // Go
    // -----------------------------------------------------------------------

    #[test]
    fn go_gin() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/gin-gonic/gin"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::Gin);
        assert_eq!(r.default_port, 8080);
    }

    #[test]
    fn go_echo() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/labstack/echo"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::Echo);
    }

    #[test]
    fn go_fiber() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/gofiber/fiber/v2"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::Fiber);
    }

    #[test]
    fn go_chi() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/go-chi/chi/v5"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::Chi);
    }

    #[test]
    fn go_generic_fallback() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/some/lib"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::GoGeneric);
    }

    #[test]
    fn go_build_cmd() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/gin-gonic/gin"]),
            &Language::Go,
        );
        assert!(r.default_build_cmd.is_some());
        assert!(r.default_build_cmd.unwrap().contains("go build"));
    }

    #[test]
    fn go_cgo_disabled() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/gin-gonic/gin"]),
            &Language::Go,
        );
        assert!(r
            .env_vars
            .iter()
            .any(|(k, v)| k == "CGO_ENABLED" && v == "0"));
    }

    #[test]
    fn go_no_web_framework_falls_to_generic() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/some/lib"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::GoGeneric);
    }

    // -----------------------------------------------------------------------
    // Rust
    // -----------------------------------------------------------------------

    #[test]
    fn rust_axum() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["axum", "tokio"]),
            &Language::Rust,
        );
        assert_eq!(r.framework, Framework::Axum);
        assert_eq!(r.default_port, 8080);
    }

    #[test]
    fn rust_actix() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["actix-web"]),
            &Language::Rust,
        );
        assert_eq!(r.framework, Framework::ActixWeb);
    }

    #[test]
    fn rust_rocket() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["rocket"]),
            &Language::Rust,
        );
        assert_eq!(r.framework, Framework::Rocket);
        assert_eq!(r.default_port, 8000);
    }

    #[test]
    fn rust_warp() {
        let r = detect_framework(tmp().path(), &manifest_with(vec!["warp"]), &Language::Rust);
        assert_eq!(r.framework, Framework::Warp);
    }

    #[test]
    fn rust_generic_fallback() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["serde", "rand"]),
            &Language::Rust,
        );
        assert_eq!(r.framework, Framework::RustGeneric);
    }

    #[test]
    fn rust_build_cmd() {
        let r = detect_framework(tmp().path(), &manifest_with(vec!["axum"]), &Language::Rust);
        assert_eq!(
            r.default_build_cmd.as_deref(),
            Some("cargo build --release")
        );
    }

    #[test]
    fn rust_env_var() {
        let r = detect_framework(tmp().path(), &manifest_with(vec!["axum"]), &Language::Rust);
        assert!(r
            .env_vars
            .iter()
            .any(|(k, v)| k == "RUST_LOG" && v == "info"));
    }

    // -----------------------------------------------------------------------
    // Java
    // -----------------------------------------------------------------------

    #[test]
    fn java_springboot_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["org.springframework.boot:spring-boot-starter-web"]),
            &Language::Java,
        );
        assert_eq!(r.framework, Framework::SpringBoot);
        assert_eq!(r.default_port, 8080);
    }

    #[test]
    fn java_springboot_by_pom() {
        let d = tmp();
        fs::write(
            d.path().join("pom.xml"),
            "<project><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId></dependency></project>",
        ).unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Java);
        assert_eq!(r.framework, Framework::SpringBoot);
    }

    #[test]
    fn java_quarkus() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["io.quarkus:quarkus-core"]),
            &Language::Java,
        );
        assert_eq!(r.framework, Framework::Quarkus);
    }

    #[test]
    fn java_micronaut() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["io.micronaut:micronaut-core"]),
            &Language::Java,
        );
        assert_eq!(r.framework, Framework::Micronaut);
    }

    #[test]
    fn java_generic_by_manifest() {
        let d = tmp();
        fs::write(d.path().join("pom.xml"), "<project></project>").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Java);
        assert_eq!(r.framework, Framework::JavaGeneric);
    }

    #[test]
    fn java_generic_by_gradle() {
        let d = tmp();
        fs::write(d.path().join("build.gradle"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Java);
        assert_eq!(r.framework, Framework::JavaGeneric);
    }

    #[test]
    fn java_env_var() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["org.springframework.boot:spring-boot-starter"]),
            &Language::Java,
        );
        assert!(r.env_vars.iter().any(|(k, _)| k == "JAVA_OPTS"));
    }

    // -----------------------------------------------------------------------
    // PHP
    // -----------------------------------------------------------------------

    #[test]
    fn php_laravel_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["laravel/framework"]),
            &Language::Php,
        );
        assert_eq!(r.framework, Framework::Laravel);
        assert_eq!(r.default_port, 8000);
    }

    #[test]
    fn php_laravel_by_artisan() {
        let d = tmp();
        fs::write(d.path().join("artisan"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Php);
        assert_eq!(r.framework, Framework::Laravel);
    }

    #[test]
    fn php_symfony_by_dep() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["symfony/framework-bundle"]),
            &Language::Php,
        );
        assert_eq!(r.framework, Framework::Symfony);
    }

    #[test]
    fn php_symfony_by_file() {
        let d = tmp();
        fs::create_dir_all(d.path().join("bin")).unwrap();
        fs::write(d.path().join("bin/console"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Php);
        assert_eq!(r.framework, Framework::Symfony);
    }

    #[test]
    fn php_generic_fallback() {
        let d = tmp();
        fs::write(d.path().join("composer.json"), "{}").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Php);
        assert_eq!(r.framework, Framework::PhpGeneric);
        assert_eq!(r.default_port, 80);
    }

    #[test]
    fn php_env_var() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["laravel/framework"]),
            &Language::Php,
        );
        assert!(r
            .env_vars
            .iter()
            .any(|(k, v)| k == "APP_ENV" && v == "production"));
    }

    // -----------------------------------------------------------------------
    // .NET
    // -----------------------------------------------------------------------

    #[test]
    fn dotnet_aspnetcore() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with_raw(vec![(
                "MyApp.csproj",
                "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
            )]),
            &Language::DotNet,
        );
        assert_eq!(r.framework, Framework::AspNetCore);
        assert_eq!(r.default_port, 8080);
    }

    #[test]
    fn dotnet_build_cmd() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with_raw(vec![(
                "MyApp.csproj",
                "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
            )]),
            &Language::DotNet,
        );
        assert!(r.default_build_cmd.unwrap().contains("dotnet publish"));
    }

    #[test]
    fn dotnet_env_var() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with_raw(vec![(
                "MyApp.csproj",
                "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
            )]),
            &Language::DotNet,
        );
        assert!(r.env_vars.iter().any(|(k, _)| k == "ASPNETCORE_URLS"));
    }

    // -----------------------------------------------------------------------
    // Ruby
    // -----------------------------------------------------------------------

    #[test]
    fn ruby_rails_by_dep() {
        let r = detect_framework(tmp().path(), &manifest_with(vec!["rails"]), &Language::Ruby);
        assert_eq!(r.framework, Framework::Rails);
        assert_eq!(r.default_port, 3000);
    }

    #[test]
    fn ruby_rails_by_bin_rails() {
        let d = tmp();
        fs::create_dir_all(d.path().join("bin")).unwrap();
        fs::write(d.path().join("bin/rails"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Ruby);
        assert_eq!(r.framework, Framework::Rails);
    }

    #[test]
    fn ruby_rails_by_config_application() {
        let d = tmp();
        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(d.path().join("config/application.rb"), "").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Ruby);
        assert_eq!(r.framework, Framework::Rails);
    }

    #[test]
    fn ruby_sinatra() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["sinatra"]),
            &Language::Ruby,
        );
        assert_eq!(r.framework, Framework::Sinatra);
        assert_eq!(r.default_port, 4567);
    }

    #[test]
    fn ruby_generic_fallback() {
        let d = tmp();
        fs::write(d.path().join("Gemfile"), "source 'https://rubygems.org'\n").unwrap();
        let r = detect_framework(d.path(), &ManifestInfo::default(), &Language::Ruby);
        assert_eq!(r.framework, Framework::RubyGeneric);
    }

    #[test]
    fn ruby_env_var() {
        let r = detect_framework(tmp().path(), &manifest_with(vec!["rails"]), &Language::Ruby);
        assert!(r.env_vars.iter().any(|(k, _)| k == "RAILS_ENV"));
    }

    // -----------------------------------------------------------------------
    // Generic / Unknown
    // -----------------------------------------------------------------------

    #[test]
    fn generic_fallback_empty() {
        let r = detect_framework(
            tmp().path(),
            &ManifestInfo::default(),
            &Language::Unknown("unknown".into()),
        );
        assert_eq!(r.framework, Framework::Generic);
        assert_eq!(r.default_port, 8080);
    }

    #[test]
    fn generic_has_no_build_or_start() {
        let r = detect_framework(
            tmp().path(),
            &ManifestInfo::default(),
            &Language::Unknown("unknown".into()),
        );
        assert!(r.default_build_cmd.is_none());
        assert!(r.default_start_cmd.is_none());
    }

    // -----------------------------------------------------------------------
    // Priority / precedence
    // -----------------------------------------------------------------------

    #[test]
    fn node_priority_next_over_express() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["next", "express"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::NextJs);
    }

    #[test]
    fn python_priority_fastapi_over_flask() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["fastapi", "flask"]),
            &Language::Python,
        );
        assert_eq!(r.framework, Framework::FastApi);
    }

    #[test]
    fn go_priority_gin_over_echo() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["github.com/gin-gonic/gin", "github.com/labstack/echo"]),
            &Language::Go,
        );
        assert_eq!(r.framework, Framework::Gin);
    }

    #[test]
    fn rust_priority_axum_over_actix() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["axum", "actix-web"]),
            &Language::Rust,
        );
        assert_eq!(r.framework, Framework::Axum);
    }

    #[test]
    fn java_priority_spring_over_quarkus() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec![
                "org.springframework.boot:spring-boot-starter",
                "io.quarkus:quarkus-core",
            ]),
            &Language::Java,
        );
        assert_eq!(r.framework, Framework::SpringBoot);
    }

    #[test]
    fn php_priority_laravel_over_symfony() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["laravel/framework", "symfony/framework-bundle"]),
            &Language::Php,
        );
        assert_eq!(r.framework, Framework::Laravel);
    }

    #[test]
    fn ruby_priority_rails_over_sinatra() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["rails", "sinatra"]),
            &Language::Ruby,
        );
        assert_eq!(r.framework, Framework::Rails);
    }

    // -----------------------------------------------------------------------
    // Marker file + dep coexistence
    // -----------------------------------------------------------------------

    #[test]
    fn nextjs_dep_and_config_both_present() {
        let d = tmp();
        fs::write(d.path().join("next.config.js"), "").unwrap();
        let r = detect_framework(
            d.path(),
            &manifest_with(vec!["next", "react"]),
            &Language::NodeJs,
        );
        assert_eq!(r.framework, Framework::NextJs);
    }

    #[test]
    fn django_manage_py_with_dep() {
        let d = tmp();
        fs::write(d.path().join("manage.py"), "").unwrap();
        let r = detect_framework(d.path(), &manifest_with(vec!["django"]), &Language::Python);
        assert_eq!(r.framework, Framework::Django);
    }

    #[test]
    fn laravel_artisan_with_dep() {
        let d = tmp();
        fs::write(d.path().join("artisan"), "").unwrap();
        let r = detect_framework(
            d.path(),
            &manifest_with(vec!["laravel/framework"]),
            &Language::Php,
        );
        assert_eq!(r.framework, Framework::Laravel);
    }

    // -----------------------------------------------------------------------
    // No indicators at all
    // -----------------------------------------------------------------------

    #[test]
    fn no_indicators_returns_generic() {
        let r = detect_framework(
            tmp().path(),
            &manifest_with(vec!["some-obscure-package"]),
            &Language::Unknown("unknown".into()),
        );
        assert_eq!(r.framework, Framework::Generic);
    }
}
