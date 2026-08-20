use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::models::{Language, PackageManager};

/// Detects the primary [`Language`] and [`PackageManager`] for a directory by
/// inspecting manifest and lock files.
///
/// Detection follows a strict priority matrix: manifest files are checked first
/// (they are unambiguous), then lock files disambiguate the package manager.
/// If no manifests are found, a file-extension heuristic is applied.
pub fn detect_language_and_pm(dir_path: &Path) -> (Language, PackageManager) {
    // --- Manifest-based detection (ordered by specificity) ---

    if dir_path.join("package.json").is_file() {
        return (Language::NodeJs, detect_node_pm(dir_path));
    }
    if dir_path.join("pyproject.toml").is_file()
        || dir_path.join("requirements.txt").is_file()
        || dir_path.join("Pipfile").is_file()
        || dir_path.join("setup.py").is_file()
    {
        return (Language::Python, detect_python_pm(dir_path));
    }
    if dir_path.join("Cargo.toml").is_file() {
        return (Language::Rust, PackageManager::Cargo);
    }
    if dir_path.join("go.mod").is_file() {
        return (Language::Go, PackageManager::GoModules);
    }
    if dir_path.join("pom.xml").is_file() {
        return (Language::Java, PackageManager::Maven);
    }
    if dir_path.join("build.gradle").is_file()
        || dir_path.join("build.gradle.kts").is_file()
        || dir_path.join("gradlew").is_file()
    {
        return (Language::Java, PackageManager::Gradle);
    }
    if dir_path.join("composer.json").is_file() {
        return (Language::Php, PackageManager::Composer);
    }
    if has_dotnet_manifest(dir_path) {
        return (Language::DotNet, PackageManager::Nuget);
    }
    if dir_path.join("Gemfile").is_file() {
        return (Language::Ruby, PackageManager::Bundler);
    }

    // --- Fallback: file extension heuristic ---

    detect_by_file_extensions(dir_path)
}

// ---------------------------------------------------------------------------
// Node.js package manager
// ---------------------------------------------------------------------------

fn detect_node_pm(dir: &Path) -> PackageManager {
    if dir.join("pnpm-lock.yaml").is_file() {
        return PackageManager::Pnpm;
    }
    if dir.join("yarn.lock").is_file() {
        return PackageManager::Yarn;
    }
    if dir.join("bun.lockb").is_file() || dir.join("bun.lock").is_file() {
        return PackageManager::Bun;
    }
    if dir.join("package-lock.json").is_file() {
        return PackageManager::Npm;
    }
    PackageManager::Npm
}

// ---------------------------------------------------------------------------
// Python package manager
// ---------------------------------------------------------------------------

fn detect_python_pm(dir: &Path) -> PackageManager {
    if dir.join("poetry.lock").is_file() {
        return PackageManager::Poetry;
    }
    if dir.join("Pipfile.lock").is_file() || dir.join("Pipfile").is_file() {
        return PackageManager::Pipenv;
    }
    PackageManager::Pip
}

// ---------------------------------------------------------------------------
// .NET
// ---------------------------------------------------------------------------

fn has_dotnet_manifest(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("csproj" | "fsproj" | "sln")
        )
    })
}

// ---------------------------------------------------------------------------
// File extension heuristic
// ---------------------------------------------------------------------------

fn detect_by_file_extensions(dir: &Path) -> (Language, PackageManager) {
    let mut counts = HashMap::new();
    count_extensions(dir, &mut counts, 0);

    let dominant = counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(ext, _)| ext.as_str());

    match dominant {
        Some("rs") => (Language::Rust, PackageManager::Cargo),
        Some("py") => (Language::Python, PackageManager::Pip),
        Some("go") => (Language::Go, PackageManager::GoModules),
        Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") => {
            (Language::NodeJs, PackageManager::Npm)
        }
        Some("java") => (Language::Java, PackageManager::Maven),
        Some("php") => (Language::Php, PackageManager::Composer),
        Some("cs" | "fs") => (Language::DotNet, PackageManager::Nuget),
        Some("rb") => (Language::Ruby, PackageManager::Bundler),
        _ => (Language::Unknown("unknown".into()), PackageManager::Unknown),
    }
}

/// Recursively counts source file extensions up to a bounded depth.
fn count_extensions(dir: &Path, counts: &mut HashMap<String, usize>, depth: u8) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if EXCLUDED_DIRS.contains(&name.as_str()) {
                continue;
            }
            count_extensions(&path, counts, depth + 1);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            *counts.entry(ext.to_ascii_lowercase()).or_insert(0) += 1;
        }
    }
}

const EXCLUDED_DIRS: &[&str] = &[
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
    ".cache",
    "bin",
    "obj",
];

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
    fn node_npm() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("package-lock.json"), "{}").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Npm);
    }

    #[test]
    fn node_pnpm() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Pnpm);
    }

    #[test]
    fn node_yarn() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Yarn);
    }

    #[test]
    fn node_bun_lockb() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("bun.lockb"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Bun);
    }

    #[test]
    fn node_bun_lock() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("bun.lock"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Bun);
    }

    #[test]
    fn node_fallback_to_npm() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Npm);
    }

    // -- Python -------------------------------------------------------------

    #[test]
    fn python_poetry() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("pyproject.toml"), "").unwrap();
        fs::write(tmp.path().join("poetry.lock"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Poetry);
    }

    #[test]
    fn python_pipenv_with_lock() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Pipfile"), "").unwrap();
        fs::write(tmp.path().join("Pipfile.lock"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Pipenv);
    }

    #[test]
    fn python_pipenv_no_lock() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Pipfile"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Pipenv);
    }

    #[test]
    fn python_requirements_txt() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("requirements.txt"), "flask\n").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Pip);
    }

    #[test]
    fn python_setup_py() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("setup.py"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Pip);
    }

    // -- Rust ---------------------------------------------------------------

    #[test]
    fn rust_cargo() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Rust);
        assert_eq!(pm, PackageManager::Cargo);
    }

    // -- Go -----------------------------------------------------------------

    #[test]
    fn go_modules() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module x\n").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Go);
        assert_eq!(pm, PackageManager::GoModules);
    }

    // -- Java ---------------------------------------------------------------

    #[test]
    fn java_maven() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("pom.xml"), "<project></project>").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Java);
        assert_eq!(pm, PackageManager::Maven);
    }

    #[test]
    fn java_gradle_groovy() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("build.gradle"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Java);
        assert_eq!(pm, PackageManager::Gradle);
    }

    #[test]
    fn java_gradle_kts() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("build.gradle.kts"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Java);
        assert_eq!(pm, PackageManager::Gradle);
    }

    #[test]
    fn java_gradlew() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("gradlew"), "#!/bin/sh\n").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Java);
        assert_eq!(pm, PackageManager::Gradle);
    }

    // -- PHP ----------------------------------------------------------------

    #[test]
    fn php_composer() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("composer.json"), r#"{"name": "x"}"#).unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Php);
        assert_eq!(pm, PackageManager::Composer);
    }

    // -- .NET ---------------------------------------------------------------

    #[test]
    fn dotnet_csproj() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("MyApp.csproj"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::DotNet);
        assert_eq!(pm, PackageManager::Nuget);
    }

    #[test]
    fn dotnet_fsproj() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("App.fsproj"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::DotNet);
        assert_eq!(pm, PackageManager::Nuget);
    }

    #[test]
    fn dotnet_sln() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("MyApp.sln"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::DotNet);
        assert_eq!(pm, PackageManager::Nuget);
    }

    // -- Ruby ---------------------------------------------------------------

    #[test]
    fn ruby_bundler() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("Gemfile"),
            "source 'https://rubygems.org'\n",
        )
        .unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Ruby);
        assert_eq!(pm, PackageManager::Bundler);
    }

    // -- Fallback heuristics ------------------------------------------------

    #[test]
    fn heuristic_rs_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.rs"), "").unwrap();
        fs::write(tmp.path().join("lib.rs"), "").unwrap();
        fs::write(tmp.path().join("helpers.rs"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Rust);
        assert_eq!(pm, PackageManager::Cargo);
    }

    #[test]
    fn heuristic_py_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("app.py"), "").unwrap();
        fs::write(tmp.path().join("utils.py"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Python);
        assert_eq!(pm, PackageManager::Pip);
    }

    #[test]
    fn heuristic_go_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.go"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Go);
        assert_eq!(pm, PackageManager::GoModules);
    }

    #[test]
    fn heuristic_ts_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("index.ts"), "").unwrap();
        fs::write(tmp.path().join("app.tsx"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::NodeJs);
        assert_eq!(pm, PackageManager::Npm);
    }

    #[test]
    fn heuristic_java_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Main.java"), "").unwrap();
        fs::write(tmp.path().join("App.java"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Java);
        assert_eq!(pm, PackageManager::Maven);
    }

    #[test]
    fn heuristic_php_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("index.php"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Php);
        assert_eq!(pm, PackageManager::Composer);
    }

    #[test]
    fn heuristic_cs_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Program.cs"), "").unwrap();
        fs::write(tmp.path().join("Startup.cs"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::DotNet);
        assert_eq!(pm, PackageManager::Nuget);
    }

    #[test]
    fn heuristic_rb_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("app.rb"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Ruby);
        assert_eq!(pm, PackageManager::Bundler);
    }

    #[test]
    fn empty_dir_returns_unknown() {
        let tmp = TempDir::new().unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Unknown("unknown".into()));
        assert_eq!(pm, PackageManager::Unknown);
    }

    // -- node_modules exclusion in heuristic ---------------------------------

    #[test]
    fn heuristic_skips_node_modules() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.rs"), "").unwrap();
        let nm = tmp.path().join("node_modules");
        fs::create_dir(&nm).unwrap();
        fs::write(nm.join("index.js"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Rust);
        assert_eq!(pm, PackageManager::Cargo);
    }

    // -- manifest priority over lockfile -------------------------------------

    #[test]
    fn manifest_takes_priority() {
        let tmp = TempDir::new().unwrap();
        // Go + yarn.lock present — should detect Go, not Node.
        fs::write(tmp.path().join("go.mod"), "module x\n").unwrap();
        fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        let (lang, pm) = detect_language_and_pm(tmp.path());
        assert_eq!(lang, Language::Go);
        assert_eq!(pm, PackageManager::GoModules);
    }
}
