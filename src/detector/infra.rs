use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::analyzer::dependencies::ManifestInfo;
use crate::analyzer::env::{self, InfraScanResult};
use crate::models::{InfraKind, InfraService, InfraSource};

// ---------------------------------------------------------------------------
// Priority order — lower is higher priority. First detection wins.
// ---------------------------------------------------------------------------

/// Priority order for infrastructure detection sources.
///
/// If multiple sources detect the same [`InfraKind`], the one with the
/// **lowest** priority value wins. This mirrors the invariants in AGENTS.md:
/// `EnvVar` > `PrismaSchema` > `ManifestDependency` > `ConfigFile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Priority(u8);

impl Priority {
    const ENV_VAR: Priority = Priority(0);
    const PRISMA_SCHEMA: Priority = Priority(1);
    const MANIFEST_DEP: Priority = Priority(2);
    #[allow(dead_code)]
    const CONFIG_FILE: Priority = Priority(3);
}

/// An infrastructure detection event: which kind, how it was found, and
/// its priority for deduplication.
#[derive(Debug, Clone)]
struct InfraDetection {
    kind: InfraKind,
    source: InfraSource,
    priority: Priority,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect infrastructure services across the project.
///
/// Analyses environment variables, manifest dependencies, raw manifest
/// content, and Prisma schema files to identify databases, caches, and
/// message brokers. Returns a **deduplicated, sorted** list of
/// [`InfraService`] instances ready for compose generation.
///
/// Deduplication rule: for each [`InfraKind`], the detection with the
/// **lowest** priority value wins (EnvVar > PrismaSchema >
/// ManifestDependency > ConfigFile).
pub fn detect_infrastructures(
    root_path: &Path,
    manifests: &[ManifestInfo],
    env_map: &BTreeMap<String, String>,
) -> Vec<InfraService> {
    let mut detections: Vec<InfraDetection> = Vec::new();

    // --- 1. Environment variable / URL scheme detection ---
    let env_result = env::detect_infra_connections(env_map);
    detections.extend(detect_from_env(&env_result));

    // --- 2. Manifest dependency detection ---
    detections.extend(detect_from_manifests(manifests));

    // --- 3. Prisma schema detection ---
    detections.extend(detect_from_prisma_schema(root_path));

    // --- 4. Deduplicate by InfraKind (lowest priority wins) ---
    let deduped = deduplicate_by_kind(detections);

    // --- 5. Build InfraService for each unique kind ---
    let mut services: Vec<InfraService> = deduped
        .into_iter()
        .map(|d| build_infra_service(d.kind, d.source))
        .collect();

    // --- 6. Deterministic sort by kind ---
    services.sort_by(|a, b| a.kind.cmp(&b.kind));

    services
}

// ---------------------------------------------------------------------------
// Environment variable / URL scheme detection
// ---------------------------------------------------------------------------

/// Detect infrastructure from environment variable connection strings
/// and well-known key names.
fn detect_from_env(result: &InfraScanResult) -> Vec<InfraDetection> {
    let mut detections = Vec::new();

    // URL scheme matches (highest signal — concrete connection string).
    for conn in &result.url_matches {
        if let Some(kind) = scheme_to_kind(&conn.scheme) {
            detections.push(InfraDetection {
                kind,
                source: InfraSource::EnvVar(conn.variable.clone()),
                priority: Priority::ENV_VAR,
            });
        }
    }

    // Well-known key names (no URL scheme — presence-only indicator).
    for key in &result.known_infra_keys {
        if detections.iter().any(|d| {
            matches!(
                &d.source,
                InfraSource::EnvVar(v) if v == key
            )
        }) {
            continue;
        }
        if let Some(kind) = key_to_kind(key) {
            detections.push(InfraDetection {
                kind,
                source: InfraSource::EnvVar(key.clone()),
                priority: Priority::ENV_VAR,
            });
        }
    }

    detections
}

/// Map a URL scheme string to an [`InfraKind`].
fn scheme_to_kind(scheme: &str) -> Option<InfraKind> {
    match scheme {
        "postgres://" | "postgresql://" => Some(InfraKind::Postgres),
        "mysql://" | "mysql2://" | "mariadb://" => Some(InfraKind::Mysql),
        "redis://" | "rediss://" => Some(InfraKind::Redis),
        "mongodb://" | "mongodb+srv://" => Some(InfraKind::Mongo),
        "amqp://" | "amqps://" => Some(InfraKind::RabbitMq),
        "kafka://" => Some(InfraKind::Kafka),
        "sqlite://" | "sqlite:///" => Some(InfraKind::Sqlite),
        _ => None,
    }
}

/// Map a well-known environment variable name to an [`InfraKind`].
fn key_to_kind(key: &str) -> Option<InfraKind> {
    match key {
        // PostgreSQL
        "POSTGRES_URL" | "POSTGRES_HOST" | "POSTGRES_PORT" | "POSTGRES_DB" | "POSTGRES_USER"
        | "POSTGRES_PASSWORD" | "PGHOST" | "PGPORT" | "PGDATABASE" | "PGUSER" | "PGPASSWORD" => {
            Some(InfraKind::Postgres)
        }

        // MySQL / MariaDB
        "MYSQL_URL"
        | "MYSQL_HOST"
        | "MYSQL_PORT"
        | "MYSQL_DATABASE"
        | "MYSQL_USER"
        | "MYSQL_PASSWORD"
        | "MYSQL_ROOT_PASSWORD"
        | "MARIADB_URL"
        | "MARIADB_HOST" => Some(InfraKind::Mysql),

        // Redis
        "REDIS_URL" | "REDIS_HOST" | "REDIS_PORT" | "UPSTASH_REDIS_REST_URL" => {
            Some(InfraKind::Redis)
        }

        // MongoDB
        "MONGODB_URI" | "MONGO_URL" | "MONGO_URI" | "MONGO_HOST" | "MONGO_PORT" => {
            Some(InfraKind::Mongo)
        }

        // RabbitMQ
        "AMQP_URL" | "AMQP_HOST" | "AMQP_PORT" | "RABBITMQ_URL" | "RABBITMQ_HOST" => {
            Some(InfraKind::RabbitMq)
        }

        // Kafka
        "KAFKA_BROKERS" | "KAFKA_BOOTSTRAP_SERVERS" | "KAFKA_HOST" | "KAFKA_PORT" => {
            Some(InfraKind::Kafka)
        }

        // SQLite
        "SQLITE_URL" | "DATABASE_FILENAME" => Some(InfraKind::Sqlite),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Manifest dependency detection
// ---------------------------------------------------------------------------

/// Known dependency names (lowercased) that signal an infrastructure service.
/// Each entry is `(dependency_name, InfraKind)`.
const KNOWN_DEPS: &[(&str, InfraKind)] = &[
    // PostgreSQL
    ("pg", InfraKind::Postgres),
    ("postgres", InfraKind::Postgres),
    ("psycopg2", InfraKind::Postgres),
    ("psycopg", InfraKind::Postgres),
    ("asyncpg", InfraKind::Postgres),
    ("pgx", InfraKind::Postgres),
    ("pq", InfraKind::Postgres),
    ("tokio-postgres", InfraKind::Postgres),
    ("diesel", InfraKind::Postgres), // NOTE: also covers MySQL — see `detect_from_manifests`
    ("sqlx", InfraKind::Postgres),   // NOTE: also covers MySQL — see `detect_from_manifests`
    ("pdo_pgsql", InfraKind::Postgres),
    // MySQL
    ("mysql", InfraKind::Mysql),
    ("mysql2", InfraKind::Mysql),
    ("pymysql", InfraKind::Mysql),
    ("mysqlclient", InfraKind::Mysql),
    ("go-sql-driver/mysql", InfraKind::Mysql),
    ("pdo_mysql", InfraKind::Mysql),
    // Redis
    ("redis", InfraKind::Redis),
    ("ioredis", InfraKind::Redis),
    ("redis-py", InfraKind::Redis),
    ("aioredis", InfraKind::Redis),
    ("go-redis", InfraKind::Redis),
    ("redigo", InfraKind::Redis),
    ("predis", InfraKind::Redis),
    ("redis-rs", InfraKind::Redis),
    ("deadpool-redis", InfraKind::Redis),
    // MongoDB
    ("mongodb", InfraKind::Mongo),
    ("mongoose", InfraKind::Mongo),
    ("pymongo", InfraKind::Mongo),
    ("motor", InfraKind::Mongo),
    ("mongoengine", InfraKind::Mongo),
    ("mongo-go-driver", InfraKind::Mongo),
    ("mongodb-driver", InfraKind::Mongo),
    // RabbitMQ
    ("amqplib", InfraKind::RabbitMq),
    ("rhea", InfraKind::RabbitMq),
    ("pika", InfraKind::RabbitMq),
    ("aio-pika", InfraKind::RabbitMq),
    ("amqp091-go", InfraKind::RabbitMq),
    ("lapin", InfraKind::RabbitMq),
    ("amqp", InfraKind::RabbitMq),
    // Kafka
    ("kafkajs", InfraKind::Kafka),
    ("kafka-node", InfraKind::Kafka),
    ("confluent-kafka", InfraKind::Kafka),
    ("aiokafka", InfraKind::Kafka),
    ("kafka-python", InfraKind::Kafka),
    ("confluent-kafka-go", InfraKind::Kafka),
    ("sarama", InfraKind::Kafka),
    ("segmentio/kafka-go", InfraKind::Kafka),
    ("rdkafka", InfraKind::Kafka),
    // SQLite
    ("sqlite3", InfraKind::Sqlite),
    ("better-sqlite3", InfraKind::Sqlite),
    ("sql.js", InfraKind::Sqlite),
    ("aiosqlite", InfraKind::Sqlite),
    ("rusqlite", InfraKind::Sqlite),
];

/// Deps that may appear in multiple infrastructure kinds.
/// We resolve ambiguity by checking raw manifest content for feature flags.
const AMBIGUOUS_DEPS: &[&str] = &["diesel", "sqlx"];

/// Detect infrastructure from manifest dependencies and raw content.
fn detect_from_manifests(manifests: &[ManifestInfo]) -> Vec<InfraDetection> {
    let mut detections = Vec::new();

    for manifest in manifests {
        let all_deps: Vec<String> = manifest
            .dependencies
            .iter()
            .chain(manifest.dev_dependencies.iter())
            .cloned()
            .collect();

        for dep in &all_deps {
            let dep_lower = dep.to_lowercase();

            // Skip ambiguous deps — resolve via raw content below.
            if AMBIGUOUS_DEPS.contains(&dep_lower.as_str()) {
                continue;
            }

            if let Some(&(_, kind)) = KNOWN_DEPS.iter().find(|(name, _)| {
                // Exact match (e.g. `pg` == `pg`).
                dep_lower == *name
                    // Go module path suffix (e.g. `github.com/jackc/pgx/v5` → `pgx`).
                    || dep_lower.contains(&format!("/{name}/"))
                    || dep_lower.ends_with(&format!("/{name}"))
                    // Python/pip suffixes (e.g. `psycopg2-binary` → `psycopg2`).
                    || dep_lower.starts_with(&format!("{name}-"))
            }) {
                detections.push(InfraDetection {
                    kind,
                    source: InfraSource::ManifestDependency(dep.clone()),
                    priority: Priority::MANIFEST_DEP,
                });
            }
        }

        // Resolve ambiguous deps (diesel, sqlx) via raw content feature flags.
        for raw in manifest.raw_content.values() {
            let raw_lower = raw.to_lowercase();

            if all_deps.iter().any(|d| d.to_lowercase() == "diesel") {
                if raw_lower.contains("postgres") || raw_lower.contains("postgresql") {
                    detections.push(InfraDetection {
                        kind: InfraKind::Postgres,
                        source: InfraSource::ManifestDependency("diesel".into()),
                        priority: Priority::MANIFEST_DEP,
                    });
                }
                if raw_lower.contains("mysql") {
                    detections.push(InfraDetection {
                        kind: InfraKind::Mysql,
                        source: InfraSource::ManifestDependency("diesel".into()),
                        priority: Priority::MANIFEST_DEP,
                    });
                }
            }

            if all_deps.iter().any(|d| d.to_lowercase() == "sqlx") {
                if raw_lower.contains("postgres") || raw_lower.contains("postgresql") {
                    detections.push(InfraDetection {
                        kind: InfraKind::Postgres,
                        source: InfraSource::ManifestDependency("sqlx".into()),
                        priority: Priority::MANIFEST_DEP,
                    });
                }
                if raw_lower.contains("mysql") {
                    detections.push(InfraDetection {
                        kind: InfraKind::Mysql,
                        source: InfraSource::ManifestDependency("sqlx".into()),
                        priority: Priority::MANIFEST_DEP,
                    });
                }
            }
        }
    }

    detections
}

// ---------------------------------------------------------------------------
// Prisma schema detection
// ---------------------------------------------------------------------------

/// Prisma schema file paths to scan (relative to a service directory).
const PRISMA_SCHEMA_PATHS: &[&str] = &["prisma/schema.prisma", "schema.prisma"];

/// Detect infrastructure from Prisma schema `datasource` provider values.
fn detect_from_prisma_schema(root_path: &Path) -> Vec<InfraDetection> {
    let mut detections = Vec::new();

    for schema_rel in PRISMA_SCHEMA_PATHS {
        let schema_path = root_path.join(schema_rel);
        if let Some(kind) = parse_prisma_provider(&schema_path) {
            detections.push(InfraDetection {
                kind,
                source: InfraSource::ConfigFile(schema_rel.to_string()),
                priority: Priority::PRISMA_SCHEMA,
            });
        }
    }

    detections
}

/// Parse a Prisma schema file for `provider = "..."` in a `datasource` block.
///
/// Returns the [`InfraKind`] if a recognised provider is found, `None`
/// otherwise. Uses lightweight line-by-line scanning — no full PEG parser.
fn parse_prisma_provider(schema_path: &Path) -> Option<InfraKind> {
    let content = fs::read_to_string(schema_path).ok()?;

    let mut in_datasource = false;
    let mut brace_depth = 0u32;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect start of `datasource` block.
        if trimmed.starts_with("datasource") && trimmed.contains('{') {
            in_datasource = true;
            brace_depth = 1;
            continue;
        }

        if in_datasource {
            // Track nested braces (unlikely in datasource but defensive).
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth = brace_depth.saturating_sub(1);
                        if brace_depth == 0 {
                            in_datasource = false;
                        }
                    }
                    _ => {}
                }
            }

            // Look for `provider = "..."` or `provider="..."`.
            if let Some(idx) = trimmed.find("provider") {
                let after = &trimmed[idx + "provider".len()..];
                let after = after.trim_start();
                if let Some(stripped) = after.strip_prefix('=') {
                    let value_part = stripped.trim();
                    let value = value_part.trim_matches(|c: char| c == '"' || c == '\'');
                    return prisma_provider_to_kind(value);
                }
            }
        }
    }

    None
}

/// Map a Prisma provider string to an [`InfraKind`].
fn prisma_provider_to_kind(provider: &str) -> Option<InfraKind> {
    match provider {
        "postgresql" | "postgres" => Some(InfraKind::Postgres),
        "mysql" => Some(InfraKind::Mysql),
        "mongodb" => Some(InfraKind::Mongo),
        "sqlite" => Some(InfraKind::Sqlite),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Build InfraService
// ---------------------------------------------------------------------------

/// Construct an [`InfraService`] from an [`InfraKind`] and detection source.
fn build_infra_service(kind: InfraKind, source: InfraSource) -> InfraService {
    let mut env_vars: Vec<(String, String)> = kind
        .default_env()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Deterministic sort.
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    InfraService {
        name: kind_to_compose_name(kind),
        image: kind.default_image().to_string(),
        port: kind.default_port(),
        env_vars,
        is_attached_to_compose: kind != InfraKind::Sqlite,
        kind,
        source,
    }
}

/// Canonical compose service name for each [`InfraKind`].
fn kind_to_compose_name(kind: InfraKind) -> String {
    match kind {
        InfraKind::Postgres => "postgres",
        InfraKind::Mysql => "mysql",
        InfraKind::Redis => "redis",
        InfraKind::Mongo => "mongo",
        InfraKind::RabbitMq => "rabbitmq",
        InfraKind::Kafka => "kafka",
        InfraKind::Sqlite => "sqlite",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

/// Remove duplicate [`InfraKind`] entries, keeping the detection with the
/// **lowest** priority value (highest precedence).
fn deduplicate_by_kind(detections: Vec<InfraDetection>) -> Vec<InfraDetection> {
    let mut best: Vec<InfraDetection> = Vec::new();

    for det in detections {
        if let Some(existing) = best.iter_mut().find(|e| e.kind == det.kind) {
            if det.priority < existing.priority {
                *existing = det;
            }
        } else {
            best.push(det);
        }
    }

    best
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // Environment variable detection
    // -----------------------------------------------------------------------

    #[test]
    fn postgres_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "postgres://localhost:5432/mydb".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
        assert_eq!(services[0].name, "postgres");
        assert_eq!(services[0].port, 5432);
        assert!(services[0].is_attached_to_compose);
    }

    #[test]
    fn postgresql_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "postgresql://user:pass@host:5432/db".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn postgres_well_known_key() {
        let mut env = BTreeMap::new();
        env.insert("PGHOST".into(), "localhost".into());
        env.insert("PGDATABASE".into(), "mydb".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn mysql_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "mysql://root:secret@localhost:3306/app".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
        assert_eq!(services[0].port, 3306);
    }

    #[test]
    fn mariadb_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "MARIADB_URL".into(),
            "mariadb://root:secret@localhost:3306/app".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
    }

    #[test]
    fn redis_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert("REDIS_URL".into(), "redis://localhost:6379".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Redis);
        assert_eq!(services[0].port, 6379);
    }

    #[test]
    fn rediss_tls_scheme() {
        let mut env = BTreeMap::new();
        env.insert("REDIS_URL".into(), "rediss://:password@host:6380".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Redis);
    }

    #[test]
    fn mongo_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "MONGO_URL".into(),
            "mongodb://user:pass@localhost:27017/mydb".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mongo);
        assert_eq!(services[0].port, 27017);
    }

    #[test]
    fn mongo_srv_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "MONGODB_URI".into(),
            "mongodb+srv://cluster0.example.mongodb.net/mydb".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mongo);
    }

    #[test]
    fn rabbitmq_amqp_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "AMQP_URL".into(),
            "amqp://guest:guest@localhost:5672".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::RabbitMq);
        assert_eq!(services[0].port, 5672);
    }

    #[test]
    fn rabbitmq_amqps_scheme() {
        let mut env = BTreeMap::new();
        env.insert(
            "RABBITMQ_URL".into(),
            "amqps://user:pass@broker:5671".into(),
        );

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::RabbitMq);
    }

    #[test]
    fn kafka_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert("KAFKA_BROKERS".into(), "kafka://localhost:9092".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Kafka);
        assert_eq!(services[0].port, 9092);
    }

    #[test]
    fn kafka_well_known_key() {
        let mut env = BTreeMap::new();
        env.insert("KAFKA_BOOTSTRAP_SERVERS".into(), "localhost:9092".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Kafka);
    }

    #[test]
    fn sqlite_url_scheme() {
        let mut env = BTreeMap::new();
        env.insert("DATABASE_URL".into(), "sqlite:///tmp/app.db".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Sqlite);
        assert_eq!(services[0].port, 0);
        assert!(!services[0].is_attached_to_compose);
    }

    // -----------------------------------------------------------------------
    // Multiple detections
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_infrastructures_detected() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "postgres://localhost:5432/app".into(),
        );
        env.insert("REDIS_URL".into(), "redis://localhost:6379".into());
        env.insert("MONGO_URL".into(), "mongodb://localhost:27017/mydb".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);

        assert_eq!(services.len(), 3);

        let kinds: Vec<InfraKind> = services.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&InfraKind::Postgres));
        assert!(kinds.contains(&InfraKind::Redis));
        assert!(kinds.contains(&InfraKind::Mongo));
    }

    // -----------------------------------------------------------------------
    // Manifest dependency detection
    // -----------------------------------------------------------------------

    #[test]
    fn node_redis_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["ioredis".into(), "express".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Redis);
    }

    #[test]
    fn python_psycopg2_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["psycopg2-binary".into(), "fastapi".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn go_pgx_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["github.com/jackc/pgx/v5".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn node_mysql2_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["mysql2".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
    }

    #[test]
    fn python_pymongo_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["pymongo".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mongo);
    }

    #[test]
    fn node_amqplib_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["amqplib".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::RabbitMq);
    }

    #[test]
    fn node_kafkajs_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["kafkajs".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Kafka);
    }

    #[test]
    fn rust_rusqlite_dependency() {
        let manifest = ManifestInfo {
            dependencies: vec!["rusqlite".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Sqlite);
    }

    #[test]
    fn dev_dependencies_detected() {
        let manifest = ManifestInfo {
            dev_dependencies: vec!["ioredis".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Redis);
    }

    // -----------------------------------------------------------------------
    // Ambiguous deps (diesel, sqlx) — feature flag resolution
    // -----------------------------------------------------------------------

    #[test]
    fn diesel_with_postgres_feature() {
        let manifest = ManifestInfo {
            dependencies: vec!["diesel".into()],
            raw_content: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "Cargo.toml".into(),
                    r#"[dependencies]
diesel = { version = "2.1", features = ["postgres"] }"#
                        .into(),
                );
                m
            },
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn diesel_with_mysql_feature() {
        let manifest = ManifestInfo {
            dependencies: vec!["diesel".into()],
            raw_content: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "Cargo.toml".into(),
                    r#"[dependencies]
diesel = { version = "2.1", features = ["mysql"] }"#
                        .into(),
                );
                m
            },
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
    }

    #[test]
    fn sqlx_with_postgres_feature() {
        let manifest = ManifestInfo {
            dependencies: vec!["sqlx".into()],
            raw_content: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "Cargo.toml".into(),
                    r#"[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres"] }"#
                        .into(),
                );
                m
            },
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn sqlx_with_mysql_feature() {
        let manifest = ManifestInfo {
            dependencies: vec!["sqlx".into()],
            raw_content: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "Cargo.toml".into(),
                    r#"[dependencies]
sqlx = { version = "0.7", features = ["mysql"] }"#
                        .into(),
                );
                m
            },
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
    }

    #[test]
    fn diesel_without_feature_no_detection() {
        let manifest = ManifestInfo {
            dependencies: vec!["diesel".into()],
            raw_content: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "Cargo.toml".into(),
                    r#"[dependencies]
diesel = "2.1""#
                        .into(),
                );
                m
            },
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert!(services.is_empty());
    }

    // -----------------------------------------------------------------------
    // Prisma schema detection
    // -----------------------------------------------------------------------

    #[test]
    fn prisma_postgresql_provider() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  provider = "prisma-client-js"
}
"#,
        )
        .unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn prisma_mysql_provider() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"
datasource db {
  provider = "mysql"
  url      = env("DATABASE_URL")
}
"#,
        )
        .unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mysql);
    }

    #[test]
    fn prisma_mongodb_provider() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"
datasource db {
  provider = "mongodb"
  url      = env("MONGODB_URI")
}
"#,
        )
        .unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Mongo);
    }

    #[test]
    fn prisma_sqlite_provider() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"
datasource db {
  provider = "sqlite"
  url      = "file:./dev.db"
}
"#,
        )
        .unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Sqlite);
    }

    #[test]
    fn prisma_provider_no_spaces() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"datasource db {
  provider="postgresql"
  url=env("DATABASE_URL")
}"#,
        )
        .unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
    }

    #[test]
    fn prisma_no_schema_file_no_panic() {
        let tmp = TempDir::new().unwrap();
        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert!(services.is_empty());
    }

    #[test]
    fn prisma_empty_schema_no_detection() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(prisma_dir.join("schema.prisma"), "// empty\n").unwrap();

        let services = detect_infrastructures(tmp.path(), &[], &BTreeMap::new());
        assert!(services.is_empty());
    }

    // -----------------------------------------------------------------------
    // Deduplication
    // -----------------------------------------------------------------------

    #[test]
    fn env_overrides_manifest_dep() {
        let mut env = BTreeMap::new();
        env.insert(
            "DATABASE_URL".into(),
            "postgres://localhost:5432/app".into(),
        );

        let manifest = ManifestInfo {
            dependencies: vec!["pg".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &env);

        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
        // EnvVar detection should win over ManifestDependency.
        assert!(matches!(services[0].source, InfraSource::EnvVar(_)));
    }

    #[test]
    fn dedup_by_kind_keeps_highest_priority() {
        let mut env = BTreeMap::new();
        env.insert("REDIS_URL".into(), "redis://localhost:6379".into());

        let manifest = ManifestInfo {
            dependencies: vec!["ioredis".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &env);

        // Both env and manifest detect Redis, but only one InfraService should exist.
        let redis_count = services
            .iter()
            .filter(|s| s.kind == InfraKind::Redis)
            .count();
        assert_eq!(redis_count, 1);
        // EnvVar wins.
        assert!(matches!(services[0].source, InfraSource::EnvVar(_)));
    }

    #[test]
    fn prisma_overrides_manifest_dep() {
        let tmp = TempDir::new().unwrap();
        let prisma_dir = tmp.path().join("prisma");
        std::fs::create_dir(&prisma_dir).unwrap();
        std::fs::write(
            prisma_dir.join("schema.prisma"),
            r#"
datasource db {
  provider = "postgres"
  url      = env("DATABASE_URL")
}
"#,
        )
        .unwrap();

        let manifest = ManifestInfo {
            dependencies: vec!["pg".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(tmp.path(), &[manifest], &BTreeMap::new());

        assert_eq!(services.len(), 1);
        assert_eq!(services[0].kind, InfraKind::Postgres);
        // PrismaSchema wins over ManifestDependency.
        assert!(matches!(services[0].source, InfraSource::ConfigFile(_)));
    }

    // -----------------------------------------------------------------------
    // Sorting & determinism
    // -----------------------------------------------------------------------

    #[test]
    fn services_sorted_by_kind() {
        let mut env = BTreeMap::new();
        env.insert("DATABASE_URL".into(), "postgres://localhost/app".into());
        env.insert("REDIS_URL".into(), "redis://localhost".into());
        env.insert("MONGO_URL".into(), "mongodb://localhost/db".into());
        env.insert("AMQP_URL".into(), "amqp://localhost".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);

        let kinds: Vec<InfraKind> = services.iter().map(|s| s.kind).collect();
        let mut sorted = kinds.clone();
        sorted.sort();
        assert_eq!(kinds, sorted);
    }

    #[test]
    fn env_vars_sorted_inside_service() {
        let manifest = ManifestInfo {
            dependencies: vec!["pg".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());

        let svc = &services[0];
        let keys: Vec<&str> = svc.env_vars.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys);
    }

    // -----------------------------------------------------------------------
    // Empty / no-op cases
    // -----------------------------------------------------------------------

    #[test]
    fn no_env_no_manifests() {
        let services = detect_infrastructures(Path::new("."), &[], &BTreeMap::new());
        assert!(services.is_empty());
    }

    #[test]
    fn unrelated_manifests_no_detection() {
        let manifest = ManifestInfo {
            dependencies: vec!["express".into(), "lodash".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert!(services.is_empty());
    }

    #[test]
    fn unrelated_env_keys_no_detection() {
        let mut env = BTreeMap::new();
        env.insert("NODE_ENV".into(), "production".into());
        env.insert("PORT".into(), "3000".into());

        let services = detect_infrastructures(Path::new("."), &[], &env);
        assert!(services.is_empty());
    }

    // -----------------------------------------------------------------------
    // InfraService fields
    // -----------------------------------------------------------------------

    #[test]
    fn postgres_default_env_vars() {
        let manifest = ManifestInfo {
            dependencies: vec!["pg".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        let svc = &services[0];

        assert_eq!(svc.image, "postgres:16-alpine");
        assert!(svc
            .env_vars
            .iter()
            .any(|(k, v)| k == "POSTGRES_USER" && v == "app"));
        assert!(svc
            .env_vars
            .iter()
            .any(|(k, v)| k == "POSTGRES_PASSWORD" && v == "postgres"));
        assert!(svc
            .env_vars
            .iter()
            .any(|(k, v)| k == "POSTGRES_DB" && v == "app"));
    }

    #[test]
    fn redis_no_default_env_vars() {
        let manifest = ManifestInfo {
            dependencies: vec!["redis".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        let svc = &services[0];

        assert_eq!(svc.image, "redis:7-alpine");
        assert!(svc.env_vars.is_empty());
    }

    #[test]
    fn sqlite_not_attached_to_compose() {
        let manifest = ManifestInfo {
            dependencies: vec!["rusqlite".into()],
            ..Default::default()
        };

        let services = detect_infrastructures(Path::new("."), &[manifest], &BTreeMap::new());
        assert!(!services[0].is_attached_to_compose);
    }

    // -----------------------------------------------------------------------
    // Deterministic output across runs
    // -----------------------------------------------------------------------

    #[test]
    fn identical_input_identical_output() {
        let mut env = BTreeMap::new();
        env.insert("DATABASE_URL".into(), "postgres://localhost/app".into());
        env.insert("REDIS_URL".into(), "redis://localhost".into());

        let manifest = ManifestInfo {
            dependencies: vec!["ioredis".into(), "pg".into()],
            ..Default::default()
        };

        let a = detect_infrastructures(Path::new("."), std::slice::from_ref(&manifest), &env);
        let b = detect_infrastructures(Path::new("."), std::slice::from_ref(&manifest), &env);

        assert_eq!(a.len(), b.len());
        for (sa, sb) in a.iter().zip(b.iter()) {
            assert_eq!(sa.kind, sb.kind);
            assert_eq!(sa.name, sb.name);
            assert_eq!(sa.image, sb.image);
            assert_eq!(sa.port, sb.port);
            assert_eq!(sa.env_vars, sb.env_vars);
            assert_eq!(sa.is_attached_to_compose, sb.is_attached_to_compose);
        }
    }

    // -----------------------------------------------------------------------
    // scheme_to_kind / key_to_kind helpers
    // -----------------------------------------------------------------------

    #[test]
    fn all_schemes_map_to_some() {
        let schemes = [
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
        for scheme in &schemes {
            assert!(
                scheme_to_kind(scheme).is_some(),
                "scheme_to_kind returned None for {scheme}"
            );
        }
    }

    #[test]
    fn unknown_scheme_returns_none() {
        assert!(scheme_to_kind("ftp://").is_none());
        assert!(scheme_to_kind("http://").is_none());
        assert!(scheme_to_kind("foobar://").is_none());
    }
}
