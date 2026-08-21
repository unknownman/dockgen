use rust_embed::RustEmbed;
use tera::Tera;

use crate::models::{Framework, Language};

// ---------------------------------------------------------------------------
// Embedded template assets
// ---------------------------------------------------------------------------

#[derive(RustEmbed)]
#[folder = "templates/"]
pub struct TemplateAssets;

/// Read the raw content of an embedded template file by its path.
///
/// The `file_path` must match the asset path exactly (e.g.
/// `"dockerfile/node/nextjs.tera"`).
pub fn get_template_content(file_path: &str) -> Option<String> {
    TemplateAssets::get(file_path).and_then(|data| {
        let content = data.data;
        String::from_utf8(content.to_vec()).ok()
    })
}

// ---------------------------------------------------------------------------
// Tera engine construction
// ---------------------------------------------------------------------------

/// Build a fully-initialised `Tera` engine with every embedded template
/// registered.
///
/// Template names match their asset paths (e.g. `"dockerfile/node/nextjs.tera"`).
pub fn create_tera_engine() -> Result<Tera, tera::Error> {
    let mut tera = Tera::default();
    for file in TemplateAssets::iter() {
        let path = file.as_ref();
        if let Some(content) = get_template_content(path) {
            tera.add_raw_template(path, &content)?;
        }
    }
    Ok(tera)
}

// ---------------------------------------------------------------------------
// Dockerfile template resolution
// ---------------------------------------------------------------------------

/// Maps a `(Language, Framework)` tuple to the embedded Dockerfile template
/// path.
pub fn resolve_dockerfile_template(language: &Language, framework: &Framework) -> &'static str {
    match language {
        Language::NodeJs => match framework {
            Framework::NextJs => "dockerfile/node/nextjs.tera",
            Framework::Nuxt => "dockerfile/node/nuxt.tera",
            Framework::NestJs => "dockerfile/node/nestjs.tera",
            Framework::SvelteKit => "dockerfile/node/sveltekit.tera",
            Framework::Remix => "dockerfile/node/remix.tera",
            Framework::Astro => "dockerfile/node/astro.tera",
            _ => "dockerfile/node/generic.tera",
        },
        Language::Python => match framework {
            Framework::FastApi => "dockerfile/python/fastapi.tera",
            Framework::Django => "dockerfile/python/django.tera",
            _ => "dockerfile/python/generic.tera",
        },
        Language::Go => match framework {
            Framework::Gin => "dockerfile/go/gin.tera",
            _ => "dockerfile/go/generic.tera",
        },
        Language::Rust => match framework {
            Framework::Axum => "dockerfile/rust/axum.tera",
            _ => "dockerfile/rust/generic.tera",
        },
        Language::Java => "dockerfile/java/springboot.tera",
        Language::Php => match framework {
            Framework::Laravel => "dockerfile/php/laravel.tera",
            _ => "dockerfile/php/generic.tera",
        },
        Language::DotNet => "dockerfile/dotnet/aspnetcore.tera",
        Language::Ruby => match framework {
            Framework::Rails => "dockerfile/ruby/rails.tera",
            _ => "dockerfile/ruby/generic.tera",
        },
        Language::Unknown(_) => "dockerfile/generic.tera",
    }
}

// ---------------------------------------------------------------------------
// Dockerignore template resolution
// ---------------------------------------------------------------------------

/// Maps a `Language` to the correct `.dockerignore` template path.
pub fn resolve_dockerignore_template(language: &Language) -> &'static str {
    match language {
        Language::NodeJs => "dockerignore/node.tera",
        Language::Python => "dockerignore/python.tera",
        Language::Go => "dockerignore/go.tera",
        Language::Rust => "dockerignore/rust.tera",
        Language::Java => "dockerignore/java.tera",
        Language::Php => "dockerignore/php.tera",
        Language::DotNet => "dockerignore/dotnet.tera",
        Language::Ruby => "dockerignore/ruby.tera",
        Language::Unknown(_) => "dockerignore/generic.tera",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- get_template_content ------------------------------------------------

    #[test]
    fn get_existing_template() {
        let content = get_template_content("dockerfile/node/nextjs.tera");
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("FROM {{ node_base }}"));
        assert!(text.contains("USER nextjs"));
    }

    #[test]
    fn get_nonexistent_template() {
        assert!(get_template_content("no/such/file.tera").is_none());
    }

    // -- create_tera_engine --------------------------------------------------

    #[test]
    fn tera_engine_loads_all_templates() {
        let tera = create_tera_engine().expect("failed to create tera engine");
        // We have 25+ templates embedded; ensure at least the minimum are loaded.
        assert!(
            tera.get_template_names().count() >= 25,
            "expected >=25 templates, got {}",
            tera.get_template_names().count()
        );
    }

    #[test]
    fn tera_engine_no_syntax_errors() {
        let tera = create_tera_engine().expect("failed to create tera engine");
        let names: Vec<String> = tera.get_template_names().map(String::from).collect();
        for name in &names {
            let tpl = tera.get_template(name).expect("template missing");
            // Access the raw source to verify it's non-empty
            let _ = tpl;
            assert!(
                get_template_content(name).is_some_and(|s| !s.is_empty()),
                "template {name} has empty source"
            );
        }
    }

    // -- resolve_dockerfile_template -----------------------------------------

    fn node_cases() -> Vec<(Framework, &'static str)> {
        vec![
            (Framework::NextJs, "dockerfile/node/nextjs.tera"),
            (Framework::Nuxt, "dockerfile/node/nuxt.tera"),
            (Framework::NestJs, "dockerfile/node/nestjs.tera"),
            (Framework::SvelteKit, "dockerfile/node/sveltekit.tera"),
            (Framework::Remix, "dockerfile/node/remix.tera"),
            (Framework::Astro, "dockerfile/node/astro.tera"),
            (Framework::Express, "dockerfile/node/generic.tera"),
            (Framework::Fastify, "dockerfile/node/generic.tera"),
            (Framework::NodeGeneric, "dockerfile/node/generic.tera"),
        ]
    }

    #[test]
    fn dockerfile_nodejs_frameworks() {
        for (fw, expected) in node_cases() {
            assert_eq!(
                resolve_dockerfile_template(&Language::NodeJs, &fw),
                expected,
                "NodeJs + {fw:?}"
            );
        }
    }

    fn python_cases() -> Vec<(Framework, &'static str)> {
        vec![
            (Framework::FastApi, "dockerfile/python/fastapi.tera"),
            (Framework::Django, "dockerfile/python/django.tera"),
            (Framework::Flask, "dockerfile/python/generic.tera"),
            (Framework::Starlette, "dockerfile/python/generic.tera"),
            (Framework::Litestar, "dockerfile/python/generic.tera"),
            (Framework::PythonGeneric, "dockerfile/python/generic.tera"),
        ]
    }

    #[test]
    fn dockerfile_python_frameworks() {
        for (fw, expected) in python_cases() {
            assert_eq!(
                resolve_dockerfile_template(&Language::Python, &fw),
                expected,
                "Python + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_go() {
        assert_eq!(
            resolve_dockerfile_template(&Language::Go, &Framework::Gin),
            "dockerfile/go/gin.tera"
        );
        for fw in [
            Framework::Echo,
            Framework::Fiber,
            Framework::Chi,
            Framework::GoGeneric,
        ] {
            assert_eq!(
                resolve_dockerfile_template(&Language::Go, &fw),
                "dockerfile/go/generic.tera",
                "Go + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_rust() {
        assert_eq!(
            resolve_dockerfile_template(&Language::Rust, &Framework::Axum),
            "dockerfile/rust/axum.tera"
        );
        for fw in [
            Framework::ActixWeb,
            Framework::Rocket,
            Framework::Warp,
            Framework::RustGeneric,
        ] {
            assert_eq!(
                resolve_dockerfile_template(&Language::Rust, &fw),
                "dockerfile/rust/generic.tera",
                "Rust + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_java() {
        for fw in [
            Framework::SpringBoot,
            Framework::Quarkus,
            Framework::Micronaut,
            Framework::JavaGeneric,
        ] {
            assert_eq!(
                resolve_dockerfile_template(&Language::Java, &fw),
                "dockerfile/java/springboot.tera",
                "Java + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_php() {
        assert_eq!(
            resolve_dockerfile_template(&Language::Php, &Framework::Laravel),
            "dockerfile/php/laravel.tera"
        );
        for fw in [Framework::Symfony, Framework::PhpGeneric] {
            assert_eq!(
                resolve_dockerfile_template(&Language::Php, &fw),
                "dockerfile/php/generic.tera",
                "Php + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_dotnet() {
        assert_eq!(
            resolve_dockerfile_template(&Language::DotNet, &Framework::AspNetCore),
            "dockerfile/dotnet/aspnetcore.tera"
        );
    }

    #[test]
    fn dockerfile_ruby() {
        assert_eq!(
            resolve_dockerfile_template(&Language::Ruby, &Framework::Rails),
            "dockerfile/ruby/rails.tera"
        );
        for fw in [Framework::Sinatra, Framework::RubyGeneric] {
            assert_eq!(
                resolve_dockerfile_template(&Language::Ruby, &fw),
                "dockerfile/ruby/generic.tera",
                "Ruby + {fw:?}"
            );
        }
    }

    #[test]
    fn dockerfile_unknown_falls_back_to_generic() {
        assert_eq!(
            resolve_dockerfile_template(&Language::Unknown("Zig".into()), &Framework::Generic),
            "dockerfile/generic.tera"
        );
    }

    // -- resolve_dockerignore_template ---------------------------------------

    #[test]
    fn dockerignore_all_languages() {
        let unknown_lang = Language::Unknown("Zig".into());
        let cases: Vec<(&Language, &str)> = vec![
            (&Language::NodeJs, "dockerignore/node.tera"),
            (&Language::Python, "dockerignore/python.tera"),
            (&Language::Go, "dockerignore/go.tera"),
            (&Language::Rust, "dockerignore/rust.tera"),
            (&Language::Java, "dockerignore/java.tera"),
            (&Language::Php, "dockerignore/php.tera"),
            (&Language::DotNet, "dockerignore/dotnet.tera"),
            (&Language::Ruby, "dockerignore/ruby.tera"),
            (&unknown_lang, "dockerignore/generic.tera"),
        ];
        for (lang, expected) in cases {
            assert_eq!(resolve_dockerignore_template(lang), expected, "{lang:?}");
        }
    }

    // -- Mock rendering ------------------------------------------------------

    fn tera_render(template_path: &str, context: &tera::Context) -> String {
        let tera = create_tera_engine().expect("tera engine init failed");
        tera.render(template_path, context).expect("render failed")
    }

    #[test]
    fn render_nextjs_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &3000);
        ctx.insert("runtime_version", &"20");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/node/nextjs.tera", &ctx);
        assert!(output.contains("FROM node:20-alpine AS deps"));
        assert!(output.contains("USER nextjs"));
        assert!(output.contains("EXPOSE 3000"));
        assert!(output.contains("PORT=3000"));
    }

    #[test]
    fn render_nextjs_dockerfile_slim() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &3000);
        ctx.insert("runtime_version", &"20");
        ctx.insert("base_image_variant", &"slim");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/node/nextjs.tera", &ctx);
        assert!(output.contains("FROM node:20-slim AS deps"));
        assert!(output.contains("USER nextjs"));
    }

    #[test]
    fn render_fastapi_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8000);
        ctx.insert("runtime_version", &"3.12");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &false);
        ctx.insert("py_short_version", &"3.12");
        let output = tera_render("dockerfile/python/fastapi.tera", &ctx);
        assert!(output.contains("FROM python:3.12-alpine AS builder"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 8000"));
        assert!(output.contains("uvicorn"));
    }

    #[test]
    fn render_fastapi_dockerfile_slim() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8000);
        ctx.insert("runtime_version", &"3.12");
        ctx.insert("base_image_variant", &"slim");
        ctx.insert("has_frontend_assets", &false);
        ctx.insert("py_short_version", &"3.12");
        let output = tera_render("dockerfile/python/fastapi.tera", &ctx);
        assert!(output.contains("FROM python:3.12-slim AS builder"));
        assert!(output.contains("USER appuser"));
    }

    #[test]
    fn render_go_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &9090);
        ctx.insert("runtime_version", &"1.22");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/go/generic.tera", &ctx);
        assert!(output.contains("FROM golang:1.22-alpine AS builder"));
        assert!(output.contains("CGO_ENABLED=0"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 9090"));
    }

    #[test]
    fn render_go_dockerfile_distroless() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &9090);
        ctx.insert("runtime_version", &"1.22");
        ctx.insert("base_image_variant", &"distroless");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/go/generic.tera", &ctx);
        assert!(output.contains("distroless"));
        assert!(output.contains("USER nonroot:nonroot"));
        assert!(output.contains("CGO_ENABLED=0"));
    }

    #[test]
    fn render_rust_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8080);
        ctx.insert("runtime_version", &"1.78");
        ctx.insert("base_image_variant", &"slim");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/rust/generic.tera", &ctx);
        assert!(output.contains("cargo-chef"));
        assert!(output.contains("cargo chef prepare"));
        assert!(output.contains("cargo chef cook"));
        assert!(output.contains("cargo build --release"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 8080"));
    }

    #[test]
    fn render_rust_dockerfile_alpine() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8080);
        ctx.insert("runtime_version", &"1.78");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/rust/generic.tera", &ctx);
        assert!(output.contains("rust:1.78-alpine"));
        assert!(output.contains("cargo-chef"));
        assert!(output.contains("USER appuser"));
    }

    #[test]
    fn render_laravel_with_frontend_assets() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8000);
        ctx.insert("runtime_version", &"8.3");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &true);
        let output = tera_render("dockerfile/php/laravel.tera", &ctx);
        assert!(output.contains("frontend-builder"));
        assert!(output.contains("npm run build"));
        assert!(output.contains("public/build"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 8000"));
    }

    #[test]
    fn render_laravel_without_frontend_assets() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8000);
        ctx.insert("runtime_version", &"8.3");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("has_frontend_assets", &false);
        let output = tera_render("dockerfile/php/laravel.tera", &ctx);
        assert!(!output.contains("frontend-builder"));
        assert!(output.contains("USER appuser"));
    }

    #[test]
    fn render_compose_file() {
        let mut ctx = tera::Context::new();
        let services = vec![
            serde_json::json!({
                "name": "api",
                "relative_path": "./services/api",
                "ports": [8080],
                "environment": [{"key": "RUST_LOG", "value": "info"}],
            }),
            serde_json::json!({
                "name": "web",
                "relative_path": "./services/web",
                "ports": [3000],
                "environment": [],
            }),
        ];
        ctx.insert("services", &services);
        let output = tera_render("compose/docker-compose.yml.tera", &ctx);
        assert!(output.contains("api:"));
        assert!(output.contains("web:"));
        assert!(output.contains("8080:8080"));
        assert!(output.contains("3000:3000"));
        assert!(output.contains("restart: unless-stopped"));
    }

    #[test]
    fn render_sveltekit_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &3000);
        ctx.insert("runtime_version", &"20");
        ctx.insert("base_image_variant", &"alpine");
        let output = tera_render("dockerfile/node/sveltekit.tera", &ctx);
        assert!(output.contains("node:20-alpine"));
        assert!(output.contains("build/index.js"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 3000"));
    }

    #[test]
    fn render_remix_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &3000);
        ctx.insert("runtime_version", &"20");
        ctx.insert("base_image_variant", &"alpine");
        let output = tera_render("dockerfile/node/remix.tera", &ctx);
        assert!(output.contains("node:20-alpine"));
        assert!(output.contains("remix-serve"));
        assert!(output.contains("build/server/index.js"));
        assert!(output.contains("USER appuser"));
    }

    #[test]
    fn render_astro_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &4321);
        ctx.insert("runtime_version", &"20");
        ctx.insert("base_image_variant", &"alpine");
        let output = tera_render("dockerfile/node/astro.tera", &ctx);
        assert!(output.contains("node:20-alpine"));
        assert!(output.contains("dist/server/entry.mjs"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 4321"));
    }

    #[test]
    fn render_gin_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8080);
        ctx.insert("runtime_version", &"1.22");
        ctx.insert("base_image_variant", &"alpine");
        let output = tera_render("dockerfile/go/gin.tera", &ctx);
        assert!(output.contains("golang:1.22-alpine"));
        assert!(output.contains("CGO_ENABLED=0"));
        assert!(output.contains("USER appuser"));
        assert!(output.contains("EXPOSE 8080"));
    }

    #[test]
    fn render_axum_dockerfile() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8080);
        ctx.insert("runtime_version", &"1.78");
        ctx.insert("base_image_variant", &"alpine");
        ctx.insert("assembly_name", &"my-api");
        let output = tera_render("dockerfile/rust/axum.tera", &ctx);
        assert!(output.contains("rust:1.78-alpine"));
        assert!(output.contains("cargo-chef"));
        assert!(output.contains("cargo build --release --bin my-api"));
        assert!(output.contains("target/release/my-api"));
        assert!(output.contains("USER appuser"));
    }

    #[test]
    fn render_rust_generic_with_assembly_name() {
        let mut ctx = tera::Context::new();
        ctx.insert("port", &8080);
        ctx.insert("runtime_version", &"1.78");
        ctx.insert("base_image_variant", &"slim");
        ctx.insert("assembly_name", &"my-server");
        let output = tera_render("dockerfile/rust/generic.tera", &ctx);
        assert!(output.contains("target/release/my-server"));
        assert!(output.contains("cargo build --release"));
        assert!(output.contains("USER appuser"));
    }
}
