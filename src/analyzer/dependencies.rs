use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// ManifestInfo
// ---------------------------------------------------------------------------

/// Aggregated metadata extracted from project manifest files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestInfo {
    /// Package / project name (e.g. `"my-api"`).
    pub package_name: Option<String>,
    /// Production dependency names (versions stripped).
    pub dependencies: Vec<String>,
    /// Dev-only dependency names.
    pub dev_dependencies: Vec<String>,
    /// Named scripts / tasks (e.g. `{"build": "next build"}`).
    pub scripts: HashMap<String, String>,
    /// Primary entrypoint file (e.g. `"main.py"`, `"src/main.rs"`).
    pub entrypoint: Option<String>,
    /// Raw contents of each manifest file that was read, keyed by filename.
    pub raw_content: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scans `dir_path` for known manifest files, parses them, and returns a
/// merged [`ManifestInfo`].
pub fn parse_directory_manifests(dir_path: &Path) -> ManifestInfo {
    let mut info = ManifestInfo::default();

    merge_opt(&mut info, &parse_package_json(dir_path));
    merge_opt(&mut info, &parse_cargo_toml(dir_path));
    merge_opt(&mut info, &parse_pyproject_toml(dir_path));
    merge_opt(&mut info, &parse_requirements_txt(dir_path));
    merge_opt(&mut info, &parse_pipfile(dir_path));
    merge_opt(&mut info, &parse_go_mod(dir_path));
    merge_opt(&mut info, &parse_composer_json(dir_path));
    merge_opt(&mut info, &parse_pom_xml(dir_path));
    merge_opt(&mut info, &parse_build_gradle(dir_path));
    merge_opt(&mut info, &parse_csproj(dir_path));
    merge_opt(&mut info, &parse_gemfile(dir_path));

    info
}

// ---------------------------------------------------------------------------
// Merge helper
// ---------------------------------------------------------------------------

fn merge_opt(base: &mut ManifestInfo, other: &Option<ManifestInfo>) {
    let Some(extra) = other else {
        return;
    };
    if base.package_name.is_none() && extra.package_name.is_some() {
        base.package_name = extra.package_name.clone();
    }
    if base.entrypoint.is_none() && extra.entrypoint.is_some() {
        base.entrypoint = extra.entrypoint.clone();
    }
    base.dependencies.extend(extra.dependencies.iter().cloned());
    base.dev_dependencies
        .extend(extra.dev_dependencies.iter().cloned());
    base.scripts.extend(extra.scripts.iter().map(|(k, v)| (k.clone(), v.clone())));
    base.raw_content
        .extend(extra.raw_content.iter().map(|(k, v)| (k.clone(), v.clone())));
}

// ---------------------------------------------------------------------------
// package.json
// ---------------------------------------------------------------------------

fn parse_package_json(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("package.json");
    let raw = fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let package_name = val.get("name").and_then(|v| v.as_str()).map(String::from);
    let entrypoint = val.get("main").and_then(|v| v.as_str()).map(String::from);

    let dependencies = extract_json_string_keys(val.get("dependencies"));
    let dev_dependencies = extract_json_string_keys(val.get("devDependencies"));

    let scripts = extract_json_string_map(val.get("scripts"));

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies,
        scripts,
        entrypoint,
        raw_content: HashMap::from([("package.json".to_string(), raw)]),
    })
}

fn extract_json_string_keys(val: Option<&serde_json::Value>) -> Vec<String> {
    let Some(obj) = val.and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    keys
}

fn extract_json_string_map(val: Option<&serde_json::Value>) -> HashMap<String, String> {
    let Some(obj) = val.and_then(|v| v.as_object()) else {
        return HashMap::new();
    };
    obj.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
}

// ---------------------------------------------------------------------------
// Cargo.toml
// ---------------------------------------------------------------------------

fn parse_cargo_toml(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("Cargo.toml");
    let raw = fs::read_to_string(&path).ok()?;
    let table: toml::Value = toml::from_str(&raw).ok()?;

    let package_name = table
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let dependencies = extract_toml_table_keys(table.get("dependencies"));
    let dev_dependencies = extract_toml_table_keys(table.get("dev-dependencies"));

    // Entrypoint: [[bin]] target or src/main.rs heuristic.
    let entrypoint = table
        .get("bin")
        .and_then(|b| {
            if let Some(arr) = b.as_array() {
                arr.first().and_then(|f| f.get("path")).and_then(|v| v.as_str()).map(String::from)
            } else {
                b.get("path").and_then(|v| v.as_str()).map(String::from)
            }
        });

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies,
        scripts: HashMap::new(),
        entrypoint,
        raw_content: HashMap::from([("Cargo.toml".to_string(), raw)]),
    })
}

fn extract_toml_table_keys(val: Option<&toml::Value>) -> Vec<String> {
    let Some(obj) = val.and_then(|v| v.as_table()) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    keys
}

// ---------------------------------------------------------------------------
// pyproject.toml
// ---------------------------------------------------------------------------

fn parse_pyproject_toml(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("pyproject.toml");
    let raw = fs::read_to_string(&path).ok()?;
    let table: toml::Value = toml::from_str(&raw).ok()?;

    let package_name = table
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // [project.dependencies] — array of strings like "fastapi>=0.100"
    let project_deps = table
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(strip_version_specifier)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // [tool.poetry.dependencies] — table with keys as package names
    let poetry_deps = extract_toml_table_keys(
        table.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.get("dependencies")),
    );

    // [tool.poetry.dev-dependencies]
    let poetry_dev_deps = extract_toml_table_keys(
        table.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.get("dev-dependencies")),
    );

    let mut dependencies = project_deps;
    dependencies.extend(poetry_deps);
    dependencies.sort();
    dependencies.dedup();

    let mut dev_dependencies = poetry_dev_deps;
    dev_dependencies.sort();
    dev_dependencies.dedup();

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies,
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("pyproject.toml".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// requirements.txt
// ---------------------------------------------------------------------------

fn parse_requirements_txt(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("requirements.txt");
    let raw = fs::read_to_string(&path).ok()?;

    let dependencies: Vec<String> = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
        .map(strip_version_specifier)
        .collect();

    Some(ManifestInfo {
        package_name: None,
        dependencies,
        dev_dependencies: Vec::new(),
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("requirements.txt".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// Pipfile
// ---------------------------------------------------------------------------

fn parse_pipfile(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("Pipfile");
    let raw = fs::read_to_string(&path).ok()?;
    let table: toml::Value = toml::from_str(&raw).ok()?;

    let dependencies = extract_toml_table_keys(table.get("packages"));
    let dev_dependencies = extract_toml_table_keys(table.get("dev-packages"));

    Some(ManifestInfo {
        package_name: None,
        dependencies,
        dev_dependencies,
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("Pipfile".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// go.mod
// ---------------------------------------------------------------------------

fn parse_go_mod(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("go.mod");
    let raw = fs::read_to_string(&path).ok()?;

    let mut dependencies = Vec::new();

    // Extract module name from `module <path>`.
    let package_name = raw.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed.strip_prefix("module ").map(|m| m.trim().to_string())
    });

    // Parse require blocks.
    let mut in_require = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("require (") || trimmed == "require" {
            in_require = true;
            // Handle single-line require without parens.
            if trimmed == "require" {
                continue;
            }
        } else if trimmed == ")" && in_require {
            in_require = false;
        } else if in_require {
            // Lines look like: "github.com/foo/bar v1.2.3"
            let pkg = trimmed.split_whitespace().next().unwrap_or("");
            if !pkg.is_empty() {
                dependencies.push(pkg.to_string());
            }
        }
    }

    dependencies.sort();

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies: Vec::new(),
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("go.mod".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// composer.json
// ---------------------------------------------------------------------------

fn parse_composer_json(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("composer.json");
    let raw = fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let package_name = val.get("name").and_then(|v| v.as_str()).map(String::from);
    let dependencies = extract_json_string_keys(val.get("require"));
    let dev_dependencies = extract_json_string_keys(val.get("require-dev"));

    let entrypoint = val
        .get("config")
        .and_then(|c| c.get("process-timeout"))
        .and(None); // composer.json doesn't have a standard entrypoint field.

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies,
        scripts: HashMap::new(),
        entrypoint,
        raw_content: HashMap::from([("composer.json".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// pom.xml
// ---------------------------------------------------------------------------

fn parse_pom_xml(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("pom.xml");
    let raw = fs::read_to_string(&path).ok()?;

    // Lightweight XML extraction — no external XML crate.
    let package_name = extract_xml_tag(&raw, "artifactId");
    let dependencies = extract_xml_dep_tags(&raw, "dependency");
    let dev_dependencies = extract_xml_dep_tags(&raw, "testDependency");

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies,
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("pom.xml".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// build.gradle / build.gradle.kts
// ---------------------------------------------------------------------------

fn parse_build_gradle(dir: &Path) -> Option<ManifestInfo> {
    let path_kts = dir.join("build.gradle.kts");
    let path_groovy = dir.join("build.gradle");

    let (path, raw) = if path_kts.is_file() {
        let raw = fs::read_to_string(&path_kts).ok()?;
        (path_kts, raw)
    } else if path_groovy.is_file() {
        let raw = fs::read_to_string(&path_groovy).ok()?;
        (path_groovy, raw)
    } else {
        return None;
    };

    let dependencies = extract_gradle_deps(&raw);

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    Some(ManifestInfo {
        package_name: None,
        dependencies,
        dev_dependencies: Vec::new(),
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([(filename, raw)]),
    })
}

// ---------------------------------------------------------------------------
// *.csproj
// ---------------------------------------------------------------------------

fn parse_csproj(dir: &Path) -> Option<ManifestInfo> {
    // Find first *.csproj in directory.
    let entries = fs::read_dir(dir).ok()?;
    let csproj_path = entries.flatten().find_map(|e| {
        let p = e.path();
        if p.extension().is_some_and(|ext| ext == "csproj") {
            Some(p)
        } else {
            None
        }
    })?;

    let raw = fs::read_to_string(&csproj_path).ok()?;
    let package_name = extract_xml_tag(&raw, "AssemblyName")
        .or_else(|| extract_xml_tag(&raw, "RootNamespace"));

    let dependencies = extract_csproj_packages(&raw, "PackageReference");

    let target_framework = extract_xml_tag(&raw, "TargetFramework");

    Some(ManifestInfo {
        package_name,
        dependencies,
        dev_dependencies: Vec::new(),
        scripts: HashMap::new(),
        entrypoint: target_framework,
        raw_content: HashMap::from([(
            csproj_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            raw,
        )]),
    })
}

// ---------------------------------------------------------------------------
// Gemfile
// ---------------------------------------------------------------------------

fn parse_gemfile(dir: &Path) -> Option<ManifestInfo> {
    let path = dir.join("Gemfile");
    let raw = fs::read_to_string(&path).ok()?;

    let dependencies: Vec<String> = raw
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Match: gem 'name' or gem "name"
            let rest = trimmed.strip_prefix("gem ")?;
            let name = rest
                .trim_start_matches(['\'', '"'])
                .split_once(['\'', '"'])
                    .map(|(name, _)| name.to_string());
            name
        })
        .collect();

    Some(ManifestInfo {
        package_name: None,
        dependencies,
        dev_dependencies: Vec::new(),
        scripts: HashMap::new(),
        entrypoint: None,
        raw_content: HashMap::from([("Gemfile".to_string(), raw)]),
    })
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Strips version specifiers from a dependency string.
///
/// `"fastapi>=0.100,<1.0"` -> `"fastapi"`
/// `"requests==2.31.0"` -> `"requests"`
/// `"click~=8.0"` -> `"click"`
fn strip_version_specifier(s: &str) -> String {
    let name = s
        .split_once(['>', '<', '=', '~', '!', '['])
        .map(|(n, _)| n.trim())
        .unwrap_or_else(|| s.trim());
    // Also strip any trailing whitespace or extras.
    let name = name.trim_end_matches(|c: char| c.is_whitespace());
    name.to_string()
}

/// Extracts a single XML tag value: `<tag>value</tag>`.
fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    let value = xml[start..start + end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Extracts `<dependency>` blocks from pom.xml, pulling `<artifactId>` values.
fn extract_xml_dep_tags(xml: &str, _tag_name: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut search_from = 0;
    while let Some(start) = xml[search_from..].find("<dependency>") {
        let abs_start = search_from + start;
        if let Some(end) = xml[abs_start..].find("</dependency>") {
            let block = &xml[abs_start..abs_start + end];
            if let Some(artifact) = extract_xml_tag(block, "artifactId") {
                deps.push(artifact);
            }
            search_from = abs_start + end + "</dependency>".len();
        } else {
            break;
        }
    }
    deps.sort();
    deps
}

/// Extracts `implementation("group:artifact:version")` from Gradle files.
fn extract_gradle_deps(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        for prefix in &["implementation", "api", "compileOnly", "runtimeOnly"] {
            let after_prefix = trimmed
                .strip_prefix(prefix)
                .and_then(|rest| {
                    let rest = rest.trim_start();
                    rest.strip_prefix('(')
                        .or(Some(rest))
                        .map(|r| r.trim())
                });
            if let Some(args) = after_prefix {
                let artifact = if args.starts_with('\'') || args.starts_with('"') {
                    args.chars().next().and_then(|quote| {
                        args.strip_prefix(quote)
                            .and_then(|inner| inner.split_once(quote))
                            .map(|(a, _)| a)
                    })
                } else {
                    args.split_once(')').map(|(a, _)| a)
                };
                if let Some(art) = artifact {
                    let coord = if art.matches(':').count() >= 2 {
                        art.rsplit_once(':').map(|(left, _)| left).unwrap_or(art)
                    } else {
                        art
                    };
                    deps.push(coord.to_string());
                }
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Extracts `<PackageReference Include="..." />` from csproj XML.
fn extract_csproj_packages(xml: &str, tag: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let needle = format!("<{tag}");
    let mut search_from = 0;
    while let Some(start) = xml[search_from..].find(&needle) {
        let abs_start = search_from + start;
        // Find the Include attribute.
        let fragment = &xml[abs_start..];
        if let Some(attr_start) = fragment.find("Include=\"") {
            let val_start = attr_start + "Include=\"".len();
            if let Some(val_end) = fragment[val_start..].find('"') {
                let name = fragment[val_start..val_start + val_end].to_string();
                deps.push(name);
            }
        }
        search_from = abs_start + needle.len();
    }
    deps.sort();
    deps
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn strip_version_specifiers() {
        assert_eq!(strip_version_specifier("fastapi>=0.100"), "fastapi");
        assert_eq!(strip_version_specifier("requests==2.31.0"), "requests");
        assert_eq!(strip_version_specifier("click~=8.0"), "click");
        assert_eq!(strip_version_specifier("django<4.0"), "django");
        assert_eq!(strip_version_specifier("numpy"), "numpy");
        assert_eq!(strip_version_specifier("pillow [extras]"), "pillow");
    }

    // -- package.json -------------------------------------------------------

    #[test]
    fn package_json_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{
                "name": "my-app",
                "main": "dist/index.js",
                "dependencies": {"react": "^18.0.0", "next": "^14.0.0"},
                "devDependencies": {"typescript": "^5.0.0"},
                "scripts": {"build": "next build", "start": "next start"}
            }"#,
        )
        .unwrap();

        let info = parse_package_json(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("my-app"));
        assert_eq!(info.entrypoint.as_deref(), Some("dist/index.js"));
        assert!(info.dependencies.contains(&"next".to_string()));
        assert!(info.dependencies.contains(&"react".to_string()));
        assert!(info.dev_dependencies.contains(&"typescript".to_string()));
        assert_eq!(info.scripts.get("build").map(|s| s.as_str()), Some("next build"));
        assert!(info.raw_content.contains_key("package.json"));
    }

    #[test]
    fn package_json_minimal() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), r#"{"name": "x"}"#).unwrap();

        let info = parse_package_json(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("x"));
        assert!(info.dependencies.is_empty());
        assert!(info.dev_dependencies.is_empty());
    }

    // -- Cargo.toml ---------------------------------------------------------

    #[test]
    fn cargo_toml_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("Cargo.toml"),
            r#"
[package]
name = "my-crate"
version = "0.1.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
"#,
        )
        .unwrap();

        let info = parse_cargo_toml(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("my-crate"));
        assert!(info.dependencies.contains(&"serde".to_string()));
        assert!(info.dependencies.contains(&"tokio".to_string()));
        assert!(info.dev_dependencies.contains(&"tempfile".to_string()));
        assert!(info.raw_content.contains_key("Cargo.toml"));
    }

    // -- pyproject.toml -----------------------------------------------------

    #[test]
    fn pyproject_toml_pep621() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pyproject.toml"),
            r#"
[project]
name = "my-python-app"
dependencies = ["fastapi>=0.100", "uvicorn[standard]>=0.20"]
"#,
        )
        .unwrap();

        let info = parse_pyproject_toml(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("my-python-app"));
        assert!(info.dependencies.contains(&"fastapi".to_string()));
        assert!(info.dependencies.contains(&"uvicorn".to_string()));
    }

    #[test]
    fn pyproject_toml_poetry() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pyproject.toml"),
            r#"
[tool.poetry.dependencies]
python = "^3.11"
django = "^4.2"

[tool.poetry.dev-dependencies]
pytest = "^7.0"
"#,
        )
        .unwrap();

        let info = parse_pyproject_toml(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"django".to_string()));
        assert!(info.dev_dependencies.contains(&"pytest".to_string()));
    }

    // -- requirements.txt ---------------------------------------------------

    #[test]
    fn requirements_txt_works() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("requirements.txt"),
            "flask==2.3.0\nrequests>=2.28\n# comment\n\nclick\n",
        )
        .unwrap();

        let info = parse_requirements_txt(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"flask".to_string()));
        assert!(info.dependencies.contains(&"requests".to_string()));
        assert!(info.dependencies.contains(&"click".to_string()));
        assert_eq!(info.dependencies.len(), 3);
    }

    // -- Pipfile ------------------------------------------------------------

    #[test]
    fn pipfile_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("Pipfile"),
            r#"
[packages]
requests = "*"
flask = ">=2.0"

[dev-packages]
pytest = "*"
"#,
        )
        .unwrap();

        let info = parse_pipfile(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"requests".to_string()));
        assert!(info.dependencies.contains(&"flask".to_string()));
        assert!(info.dev_dependencies.contains(&"pytest".to_string()));
    }

    // -- go.mod -------------------------------------------------------------

    #[test]
    fn go_mod_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("go.mod"),
            "module github.com/example/myapp\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n\tgithub.com/stretchr/testify v1.8.4\n)\n",
        )
        .unwrap();

        let info = parse_go_mod(tmp.path()).unwrap();
        assert_eq!(
            info.package_name.as_deref(),
            Some("github.com/example/myapp")
        );
        assert!(info.dependencies.contains(&"github.com/gin-gonic/gin".to_string()));
        assert!(info.dependencies.contains(&"github.com/stretchr/testify".to_string()));
    }

    // -- composer.json ------------------------------------------------------

    #[test]
    fn composer_json_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("composer.json"),
            r#"{
                "name": "myvendor/myapp",
                "require": {"php": ">=8.1", "laravel/framework": "^10.0"},
                "require-dev": {"phpunit/phpunit": "^10.0"}
            }"#,
        )
        .unwrap();

        let info = parse_composer_json(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("myvendor/myapp"));
        assert!(info.dependencies.contains(&"laravel/framework".to_string()));
        assert!(info.dev_dependencies.contains(&"phpunit/phpunit".to_string()));
    }

    // -- pom.xml ------------------------------------------------------------

    #[test]
    fn pom_xml_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pom.xml"),
            r#"<?xml version="1.0"?>
<project>
  <artifactId>my-java-app</artifactId>
  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
    <dependency>
      <groupId>com.google.code.gson</groupId>
      <artifactId>gson</artifactId>
    </dependency>
  </dependencies>
</project>"#,
        )
        .unwrap();

        let info = parse_pom_xml(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("my-java-app"));
        assert!(info.dependencies.contains(&"spring-boot-starter-web".to_string()));
        assert!(info.dependencies.contains(&"gson".to_string()));
    }

    // -- build.gradle -------------------------------------------------------

    #[test]
    fn build_gradle_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("build.gradle"),
            r#"dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web:3.1.0")
    implementation 'com.google.code.gson:gson:2.10.1'
    testImplementation("junit:junit:4.13.2")
}"#,
        )
        .unwrap();

        let info = parse_build_gradle(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"org.springframework.boot:spring-boot-starter-web".to_string()));
        assert!(info.dependencies.contains(&"com.google.code.gson:gson".to_string()));
    }

    #[test]
    fn build_gradle_kts_preferred() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("build.gradle.kts"),
            "dependencies {\n    implementation(\"org.example:lib:1.0\")\n}",
        )
        .unwrap();
        fs::write(
            tmp.path().join("build.gradle"),
            "dependencies {\n    implementation(\"org.other:lib:1.0\")\n}",
        )
        .unwrap();

        let info = parse_build_gradle(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"org.example:lib".to_string()));
    }

    // -- *.csproj -----------------------------------------------------------

    #[test]
    fn csproj_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("MyApp.csproj"),
            r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <AssemblyName>MyApp</AssemblyName>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Serilog" Version="3.0.1" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let info = parse_csproj(tmp.path()).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("MyApp"));
        assert!(info.dependencies.contains(&"Newtonsoft.Json".to_string()));
        assert!(info.dependencies.contains(&"Serilog".to_string()));
        assert_eq!(info.entrypoint.as_deref(), Some("net8.0"));
    }

    // -- Gemfile ------------------------------------------------------------

    #[test]
    fn gemfile_full() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("Gemfile"),
            "source 'https://rubygems.org'\n\ngem 'rails', '~> 7.0'\n gem 'pg'\ngem \"puma\", \">= 5.0\"\n",
        )
        .unwrap();

        let info = parse_gemfile(tmp.path()).unwrap();
        assert!(info.dependencies.contains(&"rails".to_string()));
        assert!(info.dependencies.contains(&"pg".to_string()));
        assert!(info.dependencies.contains(&"puma".to_string()));
    }

    // -- parse_directory_manifests -------------------------------------------

    #[test]
    fn parse_directory_merges_multiple() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "web", "dependencies": {"react": "^18"}}"#,
        )
        .unwrap();
        fs::write(
            tmp.path().join("requirements.txt"),
            "flask==2.3\n",
        )
        .unwrap();

        let info = parse_directory_manifests(tmp.path());
        assert_eq!(info.package_name.as_deref(), Some("web"));
        assert!(info.dependencies.contains(&"react".to_string()));
        assert!(info.dependencies.contains(&"flask".to_string()));
        assert!(info.raw_content.contains_key("package.json"));
        assert!(info.raw_content.contains_key("requirements.txt"));
    }

    // -- extract_xml_tag ----------------------------------------------------

    #[test]
    fn xml_tag_extraction() {
        let xml = "<root><name>hello</name><empty></empty></root>";
        assert_eq!(extract_xml_tag(xml, "name"), Some("hello".to_string()));
        assert_eq!(extract_xml_tag(xml, "missing"), None);
        assert_eq!(extract_xml_tag(xml, "empty"), None);
    }

    // -- extract_gradle_deps ------------------------------------------------

    #[test]
    fn gradle_dep_parsing() {
        let content = r#"dependencies {
    implementation("a.b:c:1.0")
    api 'x.y:z:2.0'
}"#;
        let deps = extract_gradle_deps(content);
        assert!(deps.contains(&"a.b:c".to_string()));
        assert!(deps.contains(&"x.y:z".to_string()));
    }
}
