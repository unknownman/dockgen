#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use regex::Regex;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `.env` file variants in iteration order. Later files override earlier ones
/// for the same key, so higher-priority files come last in the list.
///
/// Priority (highest wins): `.env.local` > `.env` > `.env.development`
/// > `.env.staging` > `.env.production` > `.env.example`.
const ENV_FILE_VARIANTS: &[&str] = &[
    ".env.example",
    ".env.production",
    ".env.staging",
    ".env.development",
    ".env",
    ".env.local",
];

/// Well-known environment variable names that commonly hold database /
/// infrastructure connection information.
const KNOWN_INFRA_KEYS: &[&str] = &[
    "DATABASE_URL",
    "DB_URL",
    "DB_HOST",
    "DB_PORT",
    "DB_NAME",
    "DB_USER",
    "DB_PASSWORD",
    "POSTGRES_URL",
    "POSTGRES_HOST",
    "POSTGRES_PORT",
    "POSTGRES_DB",
    "POSTGRES_USER",
    "POSTGRES_PASSWORD",
    "PGHOST",
    "PGPORT",
    "PGDATABASE",
    "PGUSER",
    "PGPASSWORD",
    "MYSQL_URL",
    "MYSQL_HOST",
    "MYSQL_PORT",
    "MYSQL_DATABASE",
    "MYSQL_USER",
    "MYSQL_PASSWORD",
    "MYSQL_ROOT_PASSWORD",
    "MARIADB_URL",
    "MARIADB_HOST",
    "REDIS_URL",
    "REDIS_HOST",
    "REDIS_PORT",
    "UPSTASH_REDIS_REST_URL",
    "MONGODB_URI",
    "MONGO_URL",
    "MONGO_URI",
    "MONGO_HOST",
    "MONGO_PORT",
    "AMQP_URL",
    "AMQP_HOST",
    "AMQP_PORT",
    "RABBITMQ_URL",
    "RABBITMQ_HOST",
    "KAFKA_BROKERS",
    "KAFKA_BOOTSTRAP_SERVERS",
    "KAFKA_HOST",
    "KAFKA_PORT",
    "SQLITE_URL",
    "DATABASE_FILENAME",
];

/// URL scheme prefixes that indicate an infrastructure connection string.
const URL_SCHEMES: &[&str] = &[
    "postgres://",
    "postgresql://",
    "mysql://",
    "mysql2://",
    "mariadb://",
    "redis://",
    "rediss://",
    "mongodb://",
    "mongodb+srv://",
    "amqp://",
    "amqps://",
    "kafka://",
    "sqlite://",
    "sqlite:///",
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan `dir_path` for `.env` files and return a merged, deduplicated
/// `BTreeMap<String, String>` of environment variables.
///
/// Priority order (highest wins): `.env` > `.env.local` > `.env.development`
/// > `.env.staging` > `.env.production` > `.env.example`.
///
/// Returns an empty map if `dir_path` does not exist or contains no env files.
pub fn parse_env_files(dir_path: &Path) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();

    for variant in ENV_FILE_VARIANTS {
        let file_path = dir_path.join(variant);
        if !file_path.is_file() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&file_path) {
            let parsed = parse_env_content(&content);
            // Later files override earlier ones (higher priority wins).
            for (key, value) in parsed {
                merged.insert(key, value);
            }
        }
    }

    merged
}

/// Parse a single `.env` file's content into a `BTreeMap<String, String>`.
///
/// Returns a deterministic (sorted by key) map of key-value pairs.
pub fn parse_env_content(content: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    for line in content.lines() {
        if let Some((key, value)) = parse_env_line(line) {
            map.insert(key, value);
        }
    }

    map
}

/// Represents an infrastructure connection detected from environment variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionMatch {
    /// The variable name (e.g. `DATABASE_URL`).
    pub variable: String,
    /// The detected URL scheme (e.g. `postgres://`).
    pub scheme: String,
    /// The full connection string value.
    pub value: String,
}

/// Scan environment variables for infrastructure connection strings and
/// well-known infrastructure variable names.
///
/// Returns a deduplicated, sorted list of [`ConnectionMatch`] for any URL
/// patterns found in values, plus a sorted list of well-known infra key
/// names that are present.
pub fn detect_infra_connections(env: &BTreeMap<String, String>) -> InfraScanResult {
    let scheme_re = build_scheme_regex();
    let mut url_matches = Vec::new();

    for (key, value) in env {
        if let Some(caps) = scheme_re.captures(value) {
            let scheme = caps.get(1).unwrap().as_str().to_string();
            url_matches.push(ConnectionMatch {
                variable: key.clone(),
                scheme,
                value: value.clone(),
            });
        }
    }

    // Deterministic sort by variable name.
    url_matches.sort_by(|a, b| a.variable.cmp(&b.variable));

    // Collect well-known infra keys present in the env map.
    let mut known_keys: Vec<String> = env
        .keys()
        .filter(|k| KNOWN_INFRA_KEYS.contains(&k.as_str()))
        .cloned()
        .collect();
    known_keys.sort();

    InfraScanResult {
        url_matches,
        known_infra_keys: known_keys,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Result of scanning environment variables for infrastructure indicators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfraScanResult {
    /// URL-pattern matches found in variable values.
    pub url_matches: Vec<ConnectionMatch>,
    /// Well-known infrastructure variable names present in the env map.
    pub known_infra_keys: Vec<String>,
}

/// Parse a single line from a `.env` file.
///
/// Returns `Some((key, value))` for valid lines, `None` for comments,
/// empty lines, and malformed entries.
fn parse_env_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();

    // Skip empty lines and comments.
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    // Strip leading `export ` prefix.
    let stripped = if let Some(rest) = trimmed.strip_prefix("export ") {
        rest.trim()
    } else {
        trimmed
    };

    // Find the first `=` separator.
    let eq_pos = stripped.find('=')?;
    let raw_key = stripped[..eq_pos].trim();
    let raw_value = stripped[eq_pos + 1..].trim();

    // Validate key: must be non-empty, start with a letter or underscore,
    // and contain only alphanumeric characters and underscores.
    if raw_key.is_empty() {
        return None;
    }
    let first_char = raw_key.chars().next()?;
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return None;
    }
    if !raw_key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }

    let value = strip_quotes_and_inline_comments(raw_value);

    Some((raw_key.to_string(), value))
}

/// Strip surrounding quotes (single or double) and trailing inline comments.
///
/// Handles:
/// - `"value"` → `value`
/// - `'value'` → `value`
/// - `value # comment` → `value`
/// - `"value with spaces" # comment` → `value with spaces`
fn strip_quotes_and_inline_comments(raw: &str) -> String {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return String::new();
    }

    // Strip trailing inline comment (only if not inside quotes).
    let without_comment = strip_inline_comment(trimmed);

    // Strip surrounding quotes.
    let unquoted = strip_surrounding_quotes(without_comment.trim());

    unquoted.to_string()
}

/// Strip a trailing `# ...` inline comment from a value.
///
/// If the value is quoted, the comment is only stripped if it appears after
/// the closing quote.
fn strip_inline_comment(value: &str) -> &str {
    // If the value starts with a quote, find the matching closing quote first.
    if let Some(first) = value.chars().next() {
        if first == '"' || first == '\'' {
            // Find the closing quote.
            if let Some(end) = value[1..].find(first) {
                let after_quote = &value[end + 2..];
                // If there's a `#` after the closing quote, strip it.
                if let Some(comment_pos) = after_quote.find('#') {
                    return &value[..end + 2 + comment_pos];
                }
                return value;
            }
        }
    }

    // Unquoted: find first `#` and strip from there.
    if let Some(pos) = value.find('#') {
        &value[..pos]
    } else {
        value
    }
}

/// Strip surrounding single or double quotes from a string.
fn strip_surrounding_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Build a regex that matches any of the known URL scheme prefixes.
fn build_scheme_regex() -> Regex {
    let escaped: Vec<String> = URL_SCHEMES.iter().map(|s| regex::escape(s)).collect();
    let pattern = format!("({})", escaped.join("|"));
    Regex::new(&pattern).expect("infra URL scheme regex is valid")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_env_line tests ------------------------------------------------

    #[test]
    fn parse_simple_line() {
        assert_eq!(
            parse_env_line("FOO=bar"),
            Some(("FOO".into(), "bar".into()))
        );
    }

    #[test]
    fn parse_empty_value() {
        assert_eq!(parse_env_line("FOO="), Some(("FOO".into(), String::new())));
    }

    #[test]
    fn parse_with_spaces_around_equals() {
        assert_eq!(
            parse_env_line("FOO = bar"),
            Some(("FOO".into(), "bar".into()))
        );
    }

    #[test]
    fn parse_export_prefix() {
        assert_eq!(
            parse_env_line("export DATABASE_URL=postgres://localhost/mydb"),
            Some(("DATABASE_URL".into(), "postgres://localhost/mydb".into()))
        );
    }

    #[test]
    fn parse_double_quoted_value() {
        assert_eq!(
            parse_env_line(r#"FOO="bar baz""#),
            Some(("FOO".into(), "bar baz".into()))
        );
    }

    #[test]
    fn parse_single_quoted_value() {
        assert_eq!(
            parse_env_line("FOO='bar baz'"),
            Some(("FOO".into(), "bar baz".into()))
        );
    }

    #[test]
    fn parse_inline_comment() {
        assert_eq!(
            parse_env_line("FOO=bar # this is a comment"),
            Some(("FOO".into(), "bar".into()))
        );
    }

    #[test]
    fn parse_quoted_value_with_inline_comment() {
        assert_eq!(
            parse_env_line(r#"FOO="bar baz" # comment"#),
            Some(("FOO".into(), "bar baz".into()))
        );
    }

    #[test]
    fn parse_comment_line() {
        assert_eq!(parse_env_line("# This is a comment"), None);
    }

    #[test]
    fn parse_empty_line() {
        assert_eq!(parse_env_line(""), None);
        assert_eq!(parse_env_line("   "), None);
    }

    #[test]
    fn parse_invalid_key_no_name() {
        assert_eq!(parse_env_line("=value"), None);
    }

    #[test]
    fn parse_invalid_key_starts_with_digit() {
        assert_eq!(parse_env_line("1FOO=bar"), None);
    }

    #[test]
    fn parse_invalid_key_has_dash() {
        assert_eq!(parse_env_line("FOO-BAR=baz"), None);
    }

    #[test]
    fn parse_valid_key_with_underscore() {
        assert_eq!(
            parse_env_line("MY_VAR_123=test"),
            Some(("MY_VAR_123".into(), "test".into()))
        );
    }

    #[test]
    fn parse_value_with_equals_sign() {
        assert_eq!(
            parse_env_line("FOO=bar=baz"),
            Some(("FOO".into(), "bar=baz".into()))
        );
    }

    // -- parse_env_content tests ---------------------------------------------

    #[test]
    fn parse_content_deterministic_order() {
        let content = "ZETA=1\nALPHA=2\nMU=3\n";
        let map = parse_env_content(content);
        let keys: Vec<&String> = map.keys().collect();
        assert_eq!(keys, vec!["ALPHA", "MU", "ZETA"]);
    }

    #[test]
    fn parse_content_skips_comments_and_blanks() {
        let content = "# comment\n\nFOO=bar\n   \nBAZ=qux\n";
        let map = parse_env_content(content);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(map.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn parse_content_duplicate_key_last_wins() {
        let content = "FOO=first\nFOO=second\n";
        let map = parse_env_content(content);
        assert_eq!(map.get("FOO"), Some(&"second".to_string()));
    }

    // -- parse_env_files integration tests -----------------------------------

    #[test]
    fn parse_env_files_priority_order() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // .env.example has lower priority than .env
        fs::write(dir_path.join(".env.example"), "FOO=example_val\n").unwrap();
        fs::write(dir_path.join(".env"), "FOO=env_val\n").unwrap();

        let result = parse_env_files(dir_path);
        assert_eq!(result.get("FOO"), Some(&"env_val".to_string()));
    }

    #[test]
    fn parse_env_files_local_overrides_env() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        fs::write(dir_path.join(".env"), "FOO=base\n").unwrap();
        fs::write(dir_path.join(".env.local"), "FOO=local\n").unwrap();

        let result = parse_env_files(dir_path);
        assert_eq!(result.get("FOO"), Some(&"local".to_string()));
    }

    #[test]
    fn parse_env_files_nonexistent_dir() {
        let result = parse_env_files(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_empty());
    }

    #[test]
    fn parse_env_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = parse_env_files(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn parse_env_files_merges_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // .env.example is lowest priority — written first
        fs::write(dir_path.join(".env.example"), "B=override\nC=3\n").unwrap();
        // .env overrides .env.example
        fs::write(dir_path.join(".env"), "A=1\nB=2\n").unwrap();

        let result = parse_env_files(dir_path);
        assert_eq!(result.get("A"), Some(&"1".to_string()));
        assert_eq!(result.get("B"), Some(&"2".to_string())); // .env wins
        assert_eq!(result.get("C"), Some(&"3".to_string()));
    }

    // -- Connection string detection tests -----------------------------------

    #[test]
    fn detect_postgres_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "postgres://user:pass@localhost:5432/mydb".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches.len(), 1);
        assert_eq!(result.url_matches[0].variable, "DATABASE_URL");
        assert_eq!(result.url_matches[0].scheme, "postgres://");
    }

    #[test]
    fn detect_postgresql_url() {
        let mut env = BTreeMap::new();
        env.insert("DATABASE_URL".into(), "postgresql://localhost/mydb".into());
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "postgresql://");
    }

    #[test]
    fn detect_mysql_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "MYSQL_URL".into(),
            "mysql://root:secret@127.0.0.1:3306/app".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "mysql://");
    }

    #[test]
    fn detect_redis_url() {
        let mut env = BTreeMap::new();
        env.insert("REDIS_URL".into(), "redis://localhost:6379".into());
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "redis://");
    }

    #[test]
    fn detect_rediss_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "REDIS_URL".into(),
            "rediss://default:token@host:6380".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "rediss://");
    }

    #[test]
    fn detect_mongodb_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "MONGO_URL".into(),
            "mongodb://user:pass@mongo:27017/mydb".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "mongodb://");
    }

    #[test]
    fn detect_mongodb_srv_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "MONGODB_URI".into(),
            "mongodb+srv://cluster0.example.mongodb.net".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "mongodb+srv://");
    }

    #[test]
    fn detect_amqp_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "AMQP_URL".into(),
            "amqp://guest:guest@localhost:5672".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "amqp://");
    }

    #[test]
    fn detect_amqps_url() {
        let mut env = BTreeMap::new();
        env.insert(
            "AMQP_URL".into(),
            "amqps://user:pass@rabbit.example.com".into(),
        );
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "amqps://");
    }

    #[test]
    fn detect_kafka_url() {
        let mut env = BTreeMap::new();
        env.insert("KAFKA_BROKERS".into(), "kafka://broker1:9092".into());
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches[0].scheme, "kafka://");
    }

    #[test]
    fn detect_sqlite_url() {
        let mut env = BTreeMap::new();
        env.insert("DATABASE_URL".into(), "sqlite:///./dev.db".into());
        let result = detect_infra_connections(&env);
        assert_eq!(result.url_matches[0].scheme, "sqlite://");
    }

    #[test]
    fn detect_known_infra_keys() {
        let mut env = BTreeMap::new();
        env.insert("POSTGRES_HOST".into(), "localhost".into());
        env.insert("REDIS_PORT".into(), "6379".into());
        env.insert("MY_CUSTOM_VAR".into(), "value".into());
        let result = detect_infra_connections(&env);
        assert_eq!(result.known_infra_keys, vec!["POSTGRES_HOST", "REDIS_PORT"]);
    }

    #[test]
    fn detect_no_infra() {
        let env = BTreeMap::new();
        let result = detect_infra_connections(&env);
        assert!(result.url_matches.is_empty());
        assert!(result.known_infra_keys.is_empty());
    }

    #[test]
    fn detect_multiple_urls_sorted() {
        let mut env = BTreeMap::new();
        env.insert("REDIS_URL".into(), "redis://localhost".into());
        env.insert("DATABASE_URL".into(), "postgres://localhost/db".into());
        env.insert("MONGO_URL".into(), "mongodb://localhost".into());
        let result = detect_infra_connections(&result_env(&env));
        assert_eq!(result.url_matches.len(), 3);
        // Sorted by variable name.
        assert_eq!(result.url_matches[0].variable, "DATABASE_URL");
        assert_eq!(result.url_matches[1].variable, "MONGO_URL");
        assert_eq!(result.url_matches[2].variable, "REDIS_URL");
    }

    #[test]
    fn non_url_values_ignored() {
        let mut env = BTreeMap::new();
        env.insert("FOO".into(), "not a url".into());
        env.insert("BAR".into(), "http://example.com".into()); // http not in schemes
        let result = detect_infra_connections(&result_env(&env));
        assert!(result.url_matches.is_empty());
    }

    // -- strip_quotes_and_inline_comments tests ------------------------------

    #[test]
    fn strip_double_quotes() {
        assert_eq!(strip_quotes_and_inline_comments(r#""hello""#), "hello");
    }

    #[test]
    fn strip_single_quotes() {
        assert_eq!(strip_quotes_and_inline_comments("'hello'"), "hello");
    }

    #[test]
    fn strip_inline_comment_unquoted() {
        assert_eq!(strip_quotes_and_inline_comments("value # comment"), "value");
    }

    #[test]
    fn strip_inline_comment_quoted() {
        assert_eq!(
            strip_quotes_and_inline_comments(r#""value with spaces" # comment"#),
            "value with spaces"
        );
    }

    #[test]
    fn empty_string() {
        assert_eq!(strip_quotes_and_inline_comments(""), "");
    }

    #[test]
    fn unbalanced_quote_treated_as_literal() {
        // Single quote at start but no matching close — treated as literal.
        assert_eq!(
            strip_quotes_and_inline_comments("'unbalanced"),
            "'unbalanced"
        );
    }

    // -- build_scheme_regex tests --------------------------------------------

    #[test]
    fn scheme_regex_matches_known_schemes() {
        let re = build_scheme_regex();
        assert!(re.is_match("postgres://host"));
        assert!(re.is_match("postgresql://host"));
        assert!(re.is_match("mysql://host"));
        assert!(re.is_match("redis://host"));
        assert!(re.is_match("rediss://host"));
        assert!(re.is_match("mongodb://host"));
        assert!(re.is_match("mongodb+srv://host"));
        assert!(re.is_match("amqp://host"));
        assert!(re.is_match("amqps://host"));
        assert!(re.is_match("kafka://host"));
        assert!(re.is_match("sqlite:///path"));
        assert!(!re.is_match("http://example.com"));
        assert!(!re.is_match("ftp://example.com"));
    }

    // -- Helpers -------------------------------------------------------------

    /// Helper to create a BTreeMap from an unordered HashMap-like input.
    fn result_env(input: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        input.clone()
    }
}
