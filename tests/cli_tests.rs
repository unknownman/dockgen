use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn dockgen_cmd() -> Command {
    let mut cmd = Command::cargo_bin("dockgen").unwrap();
    cmd.env("NO_COLOR", "1");
    cmd
}

// ---------------------------------------------------------------------------
// --help / --version
// ---------------------------------------------------------------------------

#[test]
fn help_flag_outputs_usage() {
    dockgen_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("dockgen"))
        .stdout(predicate::str::contains("--lang"));
}

#[test]
fn version_flag_outputs_version() {
    dockgen_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

// ---------------------------------------------------------------------------
// Mock Node.js project
// ---------------------------------------------------------------------------

#[test]
fn scan_node_project_detects_nodejs() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a minimal Node.js project with Express
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"web","scripts":{"start":"node index.js"},"dependencies":{"express":"^4.18.0"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("index.js"),
        "const express = require('express');\n",
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let services = json["analysis"]["services"]
        .as_array()
        .expect("missing services array");
    assert!(!services.is_empty(), "expected at least one service");

    let svc = &services[0];
    assert_eq!(svc["language"], "NodeJs");
    assert!(
        svc["framework"] == "Express" || svc["framework"] == "NodeGeneric",
        "expected Express or NodeGeneric, got {}",
        svc["framework"]
    );
}

// ---------------------------------------------------------------------------
// Mock Python/FastAPI project
// ---------------------------------------------------------------------------

#[test]
fn scan_python_fastapi_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("requirements.txt"),
        "fastapi==0.104.1\nuvicorn[standard]==0.24.0\n",
    )
    .unwrap();
    std::fs::write(
        root.join("main.py"),
        "from fastapi import FastAPI\napp = FastAPI()\n",
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let services = json["analysis"]["services"]
        .as_array()
        .expect("missing services array");
    assert!(!services.is_empty());

    let svc = &services[0];
    assert_eq!(svc["language"], "Python");
    assert_eq!(svc["framework"], "FastApi");
}

// ---------------------------------------------------------------------------
// --dry-run flag
// ---------------------------------------------------------------------------

#[test]
fn dry_run_does_not_write_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    dockgen_cmd().arg(root).arg("--dry-run").assert().success();

    // Verify no Dockerfile was written to disk.
    assert!(
        !root.join("Dockerfile").exists(),
        "dry-run should not write Dockerfile"
    );
}

#[test]
fn dry_run_output_contains_dockerfile_keywords() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    dockgen_cmd()
        .arg(root)
        .arg("--dry-run")
        .arg("--quiet")
        .assert()
        .success()
        .stdout(predicate::str::contains("FROM"));
}

// ---------------------------------------------------------------------------
// --json flag
// ---------------------------------------------------------------------------

#[test]
fn json_flag_outputs_valid_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Must parse as valid JSON.
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();

    // Must contain analysis.services array.
    assert!(
        json["analysis"]["services"].is_array(),
        "JSON must contain .analysis.services array"
    );
}

// ---------------------------------------------------------------------------
// --compose flag
// ---------------------------------------------------------------------------

#[test]
fn compose_flag_emits_docker_compose_yml() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .arg("--compose")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = json["files"].as_array().expect("missing files array");

    let has_compose = files.iter().any(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    assert!(
        has_compose,
        "expected docker-compose.yml in generated files"
    );
}

// ---------------------------------------------------------------------------
// Combined flags
// ---------------------------------------------------------------------------

#[test]
fn dry_run_with_compose_and_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--dry-run")
        .arg("--compose")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = json["files"].as_array().expect("missing files array");

    // Should contain both a Dockerfile and a docker-compose.yml
    let has_dockerfile = files.iter().any(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "Dockerfile")
    });
    let has_compose = files.iter().any(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    assert!(has_dockerfile, "expected Dockerfile in generated files");
    assert!(
        has_compose,
        "expected docker-compose.yml in generated files"
    );
}

// ---------------------------------------------------------------------------
// Language override
// ---------------------------------------------------------------------------

#[test]
fn language_override_rust() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"my-app\"\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--lang")
        .arg("rust")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let services = json["analysis"]["services"]
        .as_array()
        .expect("missing services array");
    assert!(!services.is_empty());
    assert_eq!(services[0]["language"], "Rust");
}

// ===========================================================================
// Phase 2 — Comprehensive E2E Integration Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// Prisma + PostgreSQL compose detection via --json
// ---------------------------------------------------------------------------

#[test]
fn test_scan_prisma_postgres_compose_json() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Minimal Node.js project with Prisma configured for PostgreSQL.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"prisma-app","scripts":{"start":"node index.js"},"dependencies":{"prisma":"^5.0.0"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('prisma app');\n").unwrap();

    // Create prisma directory and schema with postgresql provider.
    std::fs::create_dir_all(root.join("prisma")).unwrap();
    std::fs::write(
        root.join("prisma/schema.prisma"),
        r#"generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model User {
  id    Int    @id @default(autoincrement())
  email String @unique
}
"#,
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--compose")
        .arg("--json")
        .arg("-y")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();

    // Verify detected_infrastructures contains postgres.
    let infra = json["analysis"]["detected_infrastructures"]
        .as_array()
        .expect("missing detected_infrastructures array");
    let has_postgres = infra.iter().any(|i| {
        i["kind"]
            .as_str()
            .is_some_and(|k| k.eq_ignore_ascii_case("Postgres"))
    });
    assert!(
        has_postgres,
        "expected Postgres in detected_infrastructures, got: {infra:?}"
    );

    // Verify compose file is present and contains postgres service.
    let files = json["files"].as_array().expect("missing files array");
    let compose = files.iter().find(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    assert!(
        compose.is_some(),
        "expected docker-compose.yml in generated files"
    );
    let compose_content = compose.unwrap()["content"].as_str().unwrap();
    assert!(
        compose_content.contains("postgres:"),
        "compose file should contain postgres service block"
    );
    assert!(
        compose_content.contains("postgres:16-alpine"),
        "compose should use postgres:16-alpine image"
    );
}

// ---------------------------------------------------------------------------
// Env-based Redis + PostgreSQL compose detection via --dry-run
// ---------------------------------------------------------------------------

#[test]
fn test_scan_env_redis_and_postgres_compose() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Minimal Node.js project.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"env-app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    // .env file with both postgres and redis URLs.
    std::fs::write(
        root.join(".env"),
        "DATABASE_URL=postgres://user:pass@localhost:5432/myapp\nREDIS_URL=redis://localhost:6379\n",
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--compose")
        .arg("-y")
        .arg("--dry-run")
        .arg("--quiet")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Verify both infrastructure images appear in dry-run output.
    assert!(
        output_str.contains("postgres:16-alpine"),
        "expected postgres:16-alpine in output, got:\n{output_str}"
    );
    assert!(
        output_str.contains("redis:7-alpine"),
        "expected redis:7-alpine in output, got:\n{output_str}"
    );

    // Verify named volumes are declared.
    assert!(
        output_str.contains("volumes:"),
        "expected top-level volumes block in compose output"
    );
}

// ---------------------------------------------------------------------------
// Monorepo service filtering with -s
// ---------------------------------------------------------------------------

#[test]
fn test_monorepo_service_filtering() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a Turborepo-like workspace structure.
    std::fs::write(
        root.join("turbo.json"),
        r#"{"$schema":"https://turbo.build/schema.json","tasks":{}}"#,
    )
    .unwrap();

    // apps/web — Next.js project.
    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(
        root.join("apps/web/package.json"),
        r#"{"name":"web","scripts":{"build":"next build"},"dependencies":{"next":"^14.0.0","react":"^18.2.0"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("apps/web/index.tsx"),
        "export default function Home() {}\n",
    )
    .unwrap();

    // services/api — Go Gin project.
    std::fs::create_dir_all(root.join("services/api")).unwrap();
    std::fs::write(
        root.join("services/api/go.mod"),
        "module github.com/example/api\n\ngo 1.22\n\nrequire github.com/gin-gonic/gin v1.9.1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("services/api/main.go"),
        "package main\n\nfunc main() {}\n",
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .arg("-s")
        .arg("web")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let services = json["analysis"]["services"]
        .as_array()
        .expect("missing services array");

    // Only the 'web' service should be present.
    assert_eq!(
        services.len(),
        1,
        "expected exactly 1 service after filtering, got {}",
        services.len()
    );
    assert_eq!(services[0]["name"], "web");

    // Verify the api service was NOT generated.
    let files = json["files"].as_array().expect("missing files array");
    let has_api_dockerfile = files.iter().any(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p.contains("api"))
    });
    assert!(
        !has_api_dockerfile,
        "api Dockerfile should not be generated when filtered out"
    );
}

// ---------------------------------------------------------------------------
// Base image override via --base slim
// ---------------------------------------------------------------------------

#[test]
fn test_base_image_override_cli() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node.js project.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"slim-app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("-b")
        .arg("slim")
        .arg("--dry-run")
        .arg("--quiet")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // The Node.js template should use -slim base image, not -alpine.
    assert!(
        output_str.contains("node:") && output_str.contains("-slim"),
        "expected -slim base image in generated Dockerfile, got:\n{output_str}"
    );
    assert!(
        !output_str.contains("-alpine"),
        "should not contain -alpine when --base slim is used"
    );
}

// ---------------------------------------------------------------------------
// Custom port override via --port
// ---------------------------------------------------------------------------

#[test]
fn test_custom_port_flag_override() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node.js project.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"port-app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("-p")
        .arg("9090")
        .arg("--dry-run")
        .arg("--quiet")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);

    // Verify the Dockerfile exposes the custom port.
    assert!(
        output_str.contains("EXPOSE 9090"),
        "expected EXPOSE 9090 in generated Dockerfile, got:\n{output_str}"
    );
}

// ---------------------------------------------------------------------------
// Non-interactive --yes flag runs without prompting
// ---------------------------------------------------------------------------

#[test]
fn test_non_interactive_yes_flag() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node.js project with no infra — -y should just succeed silently.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"yes-app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    // --compose -y: should complete without hanging or erroring.
    dockgen_cmd()
        .arg(root)
        .arg("--compose")
        .arg("-y")
        .arg("--quiet")
        .assert()
        .success();

    // Verify files were actually written to disk.
    assert!(
        root.join("Dockerfile").exists(),
        "Dockerfile should be written when -y is used"
    );
    assert!(
        root.join("docker-compose.yml").exists(),
        "docker-compose.yml should be written when --compose -y is used"
    );
    assert!(
        root.join(".dockerignore").exists(),
        ".dockerignore should be written"
    );
}

// ---------------------------------------------------------------------------
// Compose volume declaration consistency
// ---------------------------------------------------------------------------

#[test]
fn test_compose_volume_declaration_consistency() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"vol-app","dependencies":{"pg":"^8.11.0"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    // .env with redis URL to get two infra kinds.
    std::fs::write(root.join(".env"), "REDIS_URL=redis://localhost:6379\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .arg("--compose")
        .arg("-y")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = json["files"].as_array().expect("missing files array");
    let compose = files.iter().find(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    let compose_content = compose.expect("docker-compose.yml not found")["content"]
        .as_str()
        .expect("missing content");

    // Named volumes must be declared in the top-level volumes block.
    assert!(
        compose_content.contains("volumes:"),
        "missing top-level volumes block"
    );
    assert!(
        compose_content.contains("postgresdata:"),
        "postgresdata not declared in top-level volumes block"
    );
    assert!(
        compose_content.contains("redisdata:"),
        "redisdata not declared in top-level volumes block"
    );

    // Volume mounts in service blocks must reference the declared volumes.
    assert!(
        compose_content.contains("postgresdata:/var/lib/postgresql/data"),
        "postgres volume mount missing"
    );
    assert!(
        compose_content.contains("redisdata:/data"),
        "redis volume mount missing"
    );
}

// ---------------------------------------------------------------------------
// CLI flag aliases
// ---------------------------------------------------------------------------

#[test]
fn test_cli_flag_aliases() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name":"alias-app","dependencies":{}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('hello');\n").unwrap();

    // --language alias should work identically to --lang / -l
    let output = dockgen_cmd()
        .arg(root)
        .arg("--language")
        .arg("rust")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let services = json["analysis"]["services"]
        .as_array()
        .expect("missing services array");
    assert!(!services.is_empty());
    assert_eq!(services[0]["language"], "Rust");

    // --framework alias should work identically to --fw / -f
    let output2 = dockgen_cmd()
        .arg(root)
        .arg("--framework")
        .arg("fastapi")
        .arg("--lang")
        .arg("python")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json2: serde_json::Value = serde_json::from_slice(&output2).unwrap();
    let services2 = json2["analysis"]["services"]
        .as_array()
        .expect("missing services array");
    assert!(!services2.is_empty());
    assert_eq!(services2[0]["framework"], "FastApi");

    // --force-single alias should work identically to --single
    dockgen_cmd()
        .arg(root)
        .arg("--force-single")
        .arg("--dry-run")
        .arg("--quiet")
        .assert()
        .success();

    // --output alias should work identically to --output-dir / -o
    let out_dir = tempfile::tempdir().unwrap();
    dockgen_cmd()
        .arg(root)
        .arg("--output")
        .arg(out_dir.path())
        .arg("--quiet")
        .assert()
        .success();
    assert!(
        out_dir.path().join("Dockerfile").exists(),
        "--output alias should write Dockerfile to specified directory"
    );
}

// ---------------------------------------------------------------------------
// Multi-infra compose synthesis: Postgres + Redis + RabbitMQ
// ---------------------------------------------------------------------------

#[test]
fn test_multi_infra_compose_generation() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node.js project with pg, ioredis, and amqplib dependencies.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"multi-infra","scripts":{"start":"node index.js"},"dependencies":{"pg":"^8.11.0","ioredis":"^5.3.0","amqplib":"^0.10.0"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "console.log('multi infra app');\n").unwrap();

    // .env file with postgres, redis, and rabbitmq URLs.
    std::fs::write(
        root.join(".env"),
        "DATABASE_URL=postgres://user:pass@localhost:5432/myapp\nREDIS_URL=redis://localhost:6379\nAMQP_URL=amqp://guest:guest@localhost:5672\n",
    )
    .unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .arg("--compose")
        .arg("-y")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();

    // Verify all three infra kinds are detected.
    let infra = json["analysis"]["detected_infrastructures"]
        .as_array()
        .expect("missing detected_infrastructures array");
    let detected_kinds: Vec<&str> = infra.iter().filter_map(|i| i["kind"].as_str()).collect();
    assert!(
        detected_kinds.contains(&"Postgres"),
        "expected Postgres in detected infra, got: {detected_kinds:?}"
    );
    assert!(
        detected_kinds.contains(&"Redis"),
        "expected Redis in detected infra, got: {detected_kinds:?}"
    );
    assert!(
        detected_kinds.contains(&"RabbitMq"),
        "expected RabbitMq in detected infra, got: {detected_kinds:?}"
    );

    // Verify compose file exists and contains all three service blocks.
    let files = json["files"].as_array().expect("missing files array");
    let compose = files.iter().find(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    let compose_content = compose.expect("docker-compose.yml not found")["content"]
        .as_str()
        .expect("missing content");

    // Service blocks present.
    assert!(
        compose_content.contains("postgres:"),
        "compose missing postgres service"
    );
    assert!(
        compose_content.contains("redis:"),
        "compose missing redis service"
    );
    assert!(
        compose_content.contains("rabbitmq:"),
        "compose missing rabbitmq service"
    );

    // Correct images.
    assert!(
        compose_content.contains("postgres:16-alpine"),
        "wrong postgres image"
    );
    assert!(
        compose_content.contains("redis:7-alpine"),
        "wrong redis image"
    );
    assert!(
        compose_content.contains("rabbitmq:3-management-alpine"),
        "wrong rabbitmq image"
    );

    // Port allocations.
    assert!(
        compose_content.contains("5432:5432"),
        "missing postgres port mapping"
    );
    assert!(
        compose_content.contains("6379:6379"),
        "missing redis port mapping"
    );
    assert!(
        compose_content.contains("5672:5672"),
        "missing rabbitmq port mapping"
    );

    // Volume bindings for data-persisting services.
    assert!(
        compose_content.contains("postgresdata:/var/lib/postgresql/data"),
        "missing postgres volume mount"
    );
    assert!(
        compose_content.contains("redisdata:/data"),
        "missing redis volume mount"
    );
    assert!(
        compose_content.contains("rabbitmqdata:/var/lib/rabbitmq"),
        "missing rabbitmq volume mount"
    );

    // Top-level volumes block declares all named volumes.
    assert!(
        compose_content.contains("postgresdata:"),
        "postgresdata not in top-level volumes"
    );
    assert!(
        compose_content.contains("redisdata:"),
        "redisdata not in top-level volumes"
    );
    assert!(
        compose_content.contains("rabbitmqdata:"),
        "rabbitmqdata not in top-level volumes"
    );
}

// ---------------------------------------------------------------------------
// Env + manifest combo: pg dependency in package.json triggers compose
// ---------------------------------------------------------------------------

#[test]
fn test_manifest_pg_dependency_triggers_compose() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node.js project that depends on the `pg` PostgreSQL client.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"pg-app","scripts":{"start":"node index.js"},"dependencies":{"pg":"^8.11.0"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("index.js"), "const { Client } = require('pg');\n").unwrap();

    let output = dockgen_cmd()
        .arg(root)
        .arg("--json")
        .arg("--compose")
        .arg("-y")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();

    // Verify postgres is detected via the manifest dependency.
    let infra = json["analysis"]["detected_infrastructures"]
        .as_array()
        .expect("missing detected_infrastructures array");
    let has_postgres = infra.iter().any(|i| {
        i["kind"]
            .as_str()
            .is_some_and(|k| k.eq_ignore_ascii_case("Postgres"))
    });
    assert!(
        has_postgres,
        "expected Postgres detected from pg dependency, got: {infra:?}"
    );

    // Verify compose includes the postgres service.
    let files = json["files"].as_array().expect("missing files array");
    let compose = files.iter().find(|f| {
        f["relative_path"]
            .as_str()
            .is_some_and(|p| p == "docker-compose.yml")
    });
    assert!(
        compose.is_some(),
        "expected docker-compose.yml in generated files"
    );
    let compose_content = compose.unwrap()["content"].as_str().unwrap();
    assert!(
        compose_content.contains("postgres:"),
        "compose should include postgres service"
    );
}
