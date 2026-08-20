use std::fs;
use std::path::Path;

use crate::models::Language;

/// Extracts a runtime version string for the given `language` by inspecting
/// well-known version files and manifests in `dir_path`.
///
/// Returns a cleaned version string (e.g. `"20"`, `"3.11"`, `"1.21"`, `"1.78"`)
/// or `None` if no version information could be found.
pub fn extract_runtime_version(dir_path: &Path, language: &Language) -> Option<String> {
    match language {
        Language::NodeJs => extract_node_version(dir_path),
        Language::Python => extract_python_version(dir_path),
        Language::Go => extract_go_version(dir_path),
        Language::Rust => extract_rust_version(dir_path),
        Language::Java => extract_java_version(dir_path),
        Language::Php => extract_php_version(dir_path),
        Language::DotNet => extract_dotnet_version(dir_path),
        Language::Ruby => extract_ruby_version(dir_path),
        Language::Unknown(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Node.js
// ---------------------------------------------------------------------------

fn extract_node_version(dir: &Path) -> Option<String> {
    // .nvmrc
    if let Some(v) = read_trimmed_file(&dir.join(".nvmrc")) {
        return Some(clean_node_version(&v));
    }
    // .node-version
    if let Some(v) = read_trimmed_file(&dir.join(".node-version")) {
        return Some(clean_node_version(&v));
    }
    // package.json -> engines.node
    let pkg_path = dir.join("package.json");
    if let Ok(raw) = fs::read_to_string(&pkg_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(node_ver) = val
                .get("engines")
                .and_then(|e| e.get("node"))
                .and_then(|v| v.as_str())
            {
                return Some(clean_node_version(node_ver));
            }
        }
    }
    None
}

/// Cleans a node version string: `"v20.11.0"` -> `"20"`, `"20.x"` -> `"20"`.
fn clean_node_version(raw: &str) -> String {
    let v = raw.trim().trim_start_matches('v');
    // Strip leading operators: >=, <=, ==, !=, ~, ^, >, <
    let v = v.trim_start_matches(['>', '<', '=', '!', '~', '^']);
    // Take major version only — strip `x`, `.x`, `.*`, etc.
    let major = v.split('.').next().unwrap_or(v);
    // Strip any non-numeric suffixes.
    let major = major.trim_end_matches(|c: char| !c.is_ascii_digit());
    major.to_string()
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn extract_python_version(dir: &Path) -> Option<String> {
    // .python-version
    if let Some(v) = read_trimmed_file(&dir.join(".python-version")) {
        return Some(clean_python_version(&v));
    }
    // runtime.txt (Heroku-style: "python-3.11.4")
    if let Some(v) = read_trimmed_file(&dir.join("runtime.txt")) {
        let cleaned = v.strip_prefix("python-").unwrap_or(&v).trim().to_string();
        return Some(clean_python_version(&cleaned));
    }
    // pyproject.toml -> project.requires-python or tool.poetry.dependencies.python
    let pyproject_path = dir.join("pyproject.toml");
    if let Ok(raw) = fs::read_to_string(&pyproject_path) {
        if let Ok(table) = toml::from_str::<toml::Value>(&raw) {
            // project.requires-python = ">=3.11"
            if let Some(ver) = table
                .get("project")
                .and_then(|p| p.get("requires-python"))
                .and_then(|v| v.as_str())
            {
                return Some(extract_python_version_from_spec(ver));
            }
            // tool.poetry.dependencies.python = "^3.11"
            if let Some(ver) = table
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.get("python"))
                .and_then(|v| v.as_str())
            {
                return Some(extract_python_version_from_spec(ver));
            }
        }
    }
    None
}

/// Extracts major.minor from a Python version specifier like `">=3.11"`, `"^3.11"`, `"3.11"`.
fn extract_python_version_from_spec(spec: &str) -> String {
    let s = spec.trim();
    // Strip operators: >=, <=, ==, !=, ~=, ^, >
    let stripped = s.trim_start_matches(['>', '<', '=', '!', '~', '^']);
    clean_python_version(stripped)
}

fn clean_python_version(raw: &str) -> String {
    let parts: Vec<&str> = raw.trim().split('.').collect();
    match parts.len() {
        1 => parts[0].to_string(),
        2 => format!("{}.{}", parts[0], parts[1]),
        _ => format!("{}.{}", parts[0], parts[1]),
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

fn extract_go_version(dir: &Path) -> Option<String> {
    let raw = read_trimmed_file(&dir.join("go.mod"))?;
    for line in raw.lines() {
        let trimmed = line.trim();
        // Match "go 1.21" or "go 1.21.0"
        if let Some(rest) = trimmed.strip_prefix("go ") {
            let version = rest.split_whitespace().next()?;
            let parts: Vec<&str> = version.split('.').collect();
            return match parts.len() {
                1 => Some(parts[0].to_string()),
                2 => Some(format!("{}.{}", parts[0], parts[1])),
                _ => Some(format!("{}.{}", parts[0], parts[1])),
            };
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn extract_rust_version(dir: &Path) -> Option<String> {
    // rust-toolchain.toml -> [toolchain] channel = "1.78.0"
    let tc_toml = dir.join("rust-toolchain.toml");
    if let Ok(raw) = fs::read_to_string(&tc_toml) {
        if let Ok(table) = toml::from_str::<toml::Value>(&raw) {
            if let Some(channel) = table
                .get("toolchain")
                .and_then(|t| t.get("channel"))
                .and_then(|v| v.as_str())
            {
                return Some(clean_rust_version(channel));
            }
        }
    }

    // rust-toolchain (plain text: "1.78.0" or "stable" or "nightly-2024-01-01")
    if let Some(v) = read_trimmed_file(&dir.join("rust-toolchain")) {
        // If it starts with a digit, extract semver.
        if v.starts_with(|c: char| c.is_ascii_digit()) {
            return Some(clean_rust_version(&v));
        }
        // "stable" / "nightly" — return as-is.
        return Some(v);
    }

    None
}

fn clean_rust_version(raw: &str) -> String {
    let v = raw.trim().trim_start_matches(['v', 'r']);
    let parts: Vec<&str> = v.split('.').collect();
    match parts.len() {
        0 => v.to_string(),
        1 => parts[0].to_string(),
        2 => format!("{}.{}", parts[0], parts[1]),
        _ => format!("{}.{}", parts[0], parts[1]),
    }
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

fn extract_java_version(dir: &Path) -> Option<String> {
    // pom.xml -> <java.version> or <maven.compiler.source>
    let pom_path = dir.join("pom.xml");
    if let Ok(raw) = fs::read_to_string(&pom_path) {
        if let Some(v) = extract_xml_tag_value(&raw, "java.version") {
            return Some(v);
        }
        if let Some(v) = extract_xml_tag_value(&raw, "maven.compiler.source") {
            return Some(v);
        }
    }

    // build.gradle / build.gradle.kts -> sourceCompatibility = "17" or sourceCompatibility = 17
    for name in &["build.gradle.kts", "build.gradle"] {
        let path = dir.join(name);
        if let Ok(raw) = fs::read_to_string(&path) {
            for line in raw.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("sourceCompatibility") {
                    let rest = rest.trim();
                    let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
                    // Strip quotes and trailing comment markers.
                    let value = rest
                        .trim_start_matches(['\'', '"'])
                        .trim_end_matches(['\'', '"'])
                        .split_whitespace()
                        .next()
                        .unwrap_or("");
                    // Handle "17" or "1.8" or 17 (integer literal).
                    let value = value.trim_end_matches(';');
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Lightweight XML tag value extraction: `<tag>value</tag>`.
fn extract_xml_tag_value(xml: &str, tag: &str) -> Option<String> {
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

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

fn extract_php_version(dir: &Path) -> Option<String> {
    let raw = read_trimmed_file(&dir.join("composer.json"))?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let version_str = val
        .get("require")
        .and_then(|r| r.get("php"))
        .and_then(|v| v.as_str())?;
    // Specifier like ">=8.1", "~8.1.0", "^8.1"
    let stripped = version_str.trim_start_matches(['>', '<', '=', '~', '^']);
    let parts: Vec<&str> = stripped.split('.').collect();
    match parts.len() {
        0 => Some(stripped.to_string()),
        1 => Some(parts[0].to_string()),
        2 => Some(format!("{}.{}", parts[0], parts[1])),
        _ => Some(format!("{}.{}", parts[0], parts[1])),
    }
}

// ---------------------------------------------------------------------------
// .NET
// ---------------------------------------------------------------------------

fn extract_dotnet_version(dir: &Path) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext == Some("csproj") || ext == Some("fsproj") {
            if let Ok(raw) = fs::read_to_string(&path) {
                // <TargetFramework>net8.0</TargetFramework>
                if let Some(fw) = extract_xml_tag_value(&raw, "TargetFramework") {
                    // "net8.0" -> "8.0"
                    let version = fw.strip_prefix("net").unwrap_or(&fw);
                    // Handle net8.0, netstandard2.1, etc.
                    let version = version.split('-').next().unwrap_or(version);
                    if !version.is_empty() {
                        return Some(version.to_string());
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Ruby
// ---------------------------------------------------------------------------

fn extract_ruby_version(dir: &Path) -> Option<String> {
    // .ruby-version
    if let Some(v) = read_trimmed_file(&dir.join(".ruby-version")) {
        return Some(clean_ruby_version(&v));
    }
    // Gemfile -> ruby '3.2.2'
    if let Some(raw) = read_trimmed_file(&dir.join("Gemfile")) {
        for line in raw.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("ruby ") {
                let version = rest
                    .trim_start_matches(['\'', '"'])
                    .split_once(['\'', '"'])
                    .map(|(v, _)| v.trim())
                    .unwrap_or_else(|| rest.trim());
                if !version.is_empty() {
                    return Some(clean_ruby_version(version));
                }
            }
        }
    }
    None
}

fn clean_ruby_version(raw: &str) -> String {
    let v = raw.trim();
    let parts: Vec<&str> = v.split('.').collect();
    match parts.len() {
        0 => v.to_string(),
        1 => parts[0].to_string(),
        2 => format!("{}.{}", parts[0], parts[1]),
        _ => format!("{}.{}.{}", parts[0], parts[1], parts[2]),
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Reads a file and returns its trimmed content, or `None`.
fn read_trimmed_file(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // -- Node.js ------------------------------------------------------------

    #[test]
    fn node_version_nvmrc() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".nvmrc"), "v20.11.0\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::NodeJs),
            Some("20".to_string())
        );
    }

    #[test]
    fn node_version_node_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".node-version"), "22\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::NodeJs),
            Some("22".to_string())
        );
    }

    #[test]
    fn node_version_package_json() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"engines": {"node": ">=20.0.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::NodeJs),
            Some("20".to_string())
        );
    }

    #[test]
    fn node_version_none() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(extract_runtime_version(tmp.path(), &Language::NodeJs), None);
    }

    // -- Python -------------------------------------------------------------

    #[test]
    fn python_version_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".python-version"), "3.12.1\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Python),
            Some("3.12".to_string())
        );
    }

    #[test]
    fn python_version_runtime_txt() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("runtime.txt"), "python-3.11.4\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Python),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn python_version_pyproject_pep621() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pyproject.toml"),
            "[project]\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Python),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn python_version_pyproject_poetry() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pyproject.toml"),
            "[tool.poetry.dependencies]\npython = \"^3.10\"\n",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Python),
            Some("3.10".to_string())
        );
    }

    // -- Go -----------------------------------------------------------------

    #[test]
    fn go_version_from_go_mod() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module foo\n\ngo 1.21\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Go),
            Some("1.21".to_string())
        );
    }

    #[test]
    fn go_version_three_parts() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module foo\n\ngo 1.22.1\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Go),
            Some("1.22".to_string())
        );
    }

    #[test]
    fn go_version_none() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(extract_runtime_version(tmp.path(), &Language::Go), None);
    }

    // -- Rust ---------------------------------------------------------------

    #[test]
    fn rust_version_toolchain_toml() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.78.0\"\n",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Rust),
            Some("1.78".to_string())
        );
    }

    #[test]
    fn rust_version_toolchain_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("rust-toolchain"), "1.75.0\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Rust),
            Some("1.75".to_string())
        );
    }

    #[test]
    fn rust_version_nightly() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("rust-toolchain"), "nightly\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Rust),
            Some("nightly".to_string())
        );
    }

    // -- Java ---------------------------------------------------------------

    #[test]
    fn java_version_pom_xml() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pom.xml"),
            "<project><java.version>17</java.version></project>",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Java),
            Some("17".to_string())
        );
    }

    #[test]
    fn java_version_pom_compiler_source() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("pom.xml"),
            "<project><maven.compiler.source>21</maven.compiler.source></project>",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Java),
            Some("21".to_string())
        );
    }

    #[test]
    fn java_version_gradle() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("build.gradle"),
            "sourceCompatibility = '17'\n",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Java),
            Some("17".to_string())
        );
    }

    #[test]
    fn java_version_gradle_kts() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("build.gradle.kts"),
            "sourceCompatibility = \"21\"\n",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Java),
            Some("21".to_string())
        );
    }

    // -- PHP ----------------------------------------------------------------

    #[test]
    fn php_version_composer() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("composer.json"),
            r#"{"require": {"php": ">=8.1"}}"#,
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Php),
            Some("8.1".to_string())
        );
    }

    #[test]
    fn php_version_caret() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("composer.json"),
            r#"{"require": {"php": "^8.2"}}"#,
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Php),
            Some("8.2".to_string())
        );
    }

    // -- .NET ---------------------------------------------------------------

    #[test]
    fn dotnet_version_csproj() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("MyApp.csproj"),
            "<Project><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::DotNet),
            Some("8.0".to_string())
        );
    }

    #[test]
    fn dotnet_version_fsproj() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("App.fsproj"),
            "<Project><PropertyGroup><TargetFramework>net7.0</TargetFramework></PropertyGroup></Project>",
        )
        .unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::DotNet),
            Some("7.0".to_string())
        );
    }

    // -- Ruby ---------------------------------------------------------------

    #[test]
    fn ruby_version_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".ruby-version"), "3.2.2\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Ruby),
            Some("3.2.2".to_string())
        );
    }

    #[test]
    fn ruby_version_gemfile() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Gemfile"), "ruby '3.1.4'\n").unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Ruby),
            Some("3.1.4".to_string())
        );
    }

    // -- Unknown language returns None ---------------------------------------

    #[test]
    fn unknown_language_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(
            extract_runtime_version(tmp.path(), &Language::Unknown("zig".to_string())),
            None
        );
    }

    // -- No files present returns None --------------------------------------

    #[test]
    fn empty_dir_returns_none() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(extract_runtime_version(tmp.path(), &Language::NodeJs), None);
    }
}
