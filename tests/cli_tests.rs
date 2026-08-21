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
