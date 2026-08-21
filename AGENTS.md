# AGENTS.md — DockGen Architecture & Coding Standards

## 1. Project Identity & Vision

**dockgen** — Blazing fast Rust CLI for deterministic Dockerfile, `.dockerignore`, and `docker-compose.yml` generation.

**Core Mission:** Scan any project directory, automatically detect languages, frameworks, package managers, and infrastructure dependencies, then emit production-grade, security-hardened Docker artifacts with zero manual intervention.

**CLI Capabilities:**

| Flag | Purpose |
|------|---------|
| `--dry-run` | Render all output to stdout without writing files |
| `--json` | Machine-readable JSON output (analysis + generated files) |
| `--compose` | Emit `docker-compose.yml` alongside Dockerfiles |
| `--base <variant>` | Override base image variant (`alpine`, `slim`, `distroless`) |
| `--port <ports>` | Override exposed ports per service |
| `--force-single` | Generate named `Dockerfile.<svc>` files at root (monorepo) |
| `--output <dir>` | Redirect all output to a different directory |
| `--lang <lang>` | Force language detection override |
| `--framework <fw>` | Force framework detection override |
| `--services <names>` | Filter to specific services only |
| `-i, --interactive` | Adaptive interactive Q&A wizard (Phase 2) |
| `-y, --yes` | Accept all defaults (non-interactive) |
| `-v, --verbose` | Debug-level tracing output |
| `-q, --quiet` | Suppress all non-error output |

---

## 2. Two-Phase Architectural Philosophy

DockGen operates in two distinct phases, sharing a single analysis pipeline.

### Phase 1: Conservative Heuristic Discovery (Default)

Runs automatically on every invocation. Makes **zero false-positive** assumptions — only emits outputs with high-confidence detections.

**Scope of Phase 1:**

1. **Monorepo Topology Detection** — Scans for workspace manifests (`pnpm-workspace.yaml`, `lerna.json`, `turbo.json`, `nx.json`, `Cargo.toml` `[workspace]`, `go.work`, `workspaces` in `package.json`) to classify single-service vs. monorepo.
2. **Language Detection** — Manifest files (`package.json`, `requirements.txt`, `go.mod`, `Cargo.toml`, `pom.xml`, `build.gradle`, `composer.json`, `*.csproj`, `Gemfile`) take priority; fallback to heuristic file-extension counting.
3. **Framework Detection** — Dependency analysis (npm packages, Python imports, Go module paths, Cargo features, Composer packages, Gemfile entries) + config file detection + entry-point heuristic scanning. 30+ frameworks across 8 ecosystems.
4. **Runtime Version Extraction** — Reads `.nvmrc`, `.node-version`, `.python-version`, `runtime.txt`, `go.mod` `go` directive, `[package] rust-version`, Java source/target in `pom.xml`/`build.gradle`, PHP version in `composer.json`, `Gemfile` ruby version, `.csproj` `<TargetFramework>`.
5. **Package Manager Detection** — Lockfile presence (`package-lock.json` → npm, `yarn.lock` → yarn, `pnpm-lock.yaml` → pnpm, `bun.lockb`/`bun.lock` → bun, `poetry.lock` → poetry, `Pipfile.lock` → pipenv, etc.).
6. **Service Boundary Resolution** — Well-known directories (`apps/`, `services/`, `packages/`, `cmd/`, `internal/`, `src/`) + per-candidate manifest analysis to split monorepos into discrete `Service` units.
7. **Dependency Manifest Parsing** — 11 parsers: `Cargo.toml`, `package.json`, `go.mod`, `pom.xml`, `build.gradle`/`.kts`, `requirements.txt`, `pyproject.toml` (PEP 621 + Poetry), `Pipfile`, `composer.json`, `*.csproj`, `Gemfile`. Populates `ManifestInfo` with dependencies, dev-dependencies, entrypoint, package name, and scripts.

**Zero false-positive guarantee:** If detection confidence is below threshold, a warning is emitted in `ProjectAnalysis::warnings` rather than guessing.

### Phase 2: Adaptive Interactive Q&A

Activated only when `-i` / `--interactive` is passed. Builds on Phase 1 results to ask context-driven follow-up questions.

**Activation:**
- `dockgen -i` — full interactive mode with contextual prompts
- `dockgen -i -y` — non-interactive defaults (same as `dockgen` without `-i`)
- `dockgen --json` — suppresses all interactive prompts; JSON output only

**Question Categories (triggered by Phase 1 detections):**

| Detection | Trigger | Question |
|-----------|---------|----------|
| PostgreSQL connection string or driver | `DATABASE_URL` contains `postgres` | "Add PostgreSQL to docker-compose.yml?" |
| MySQL/MariaDB driver in manifest | `mysql2`/`pymysql` in deps | "Add MySQL to docker-compose.yml?" |
| Redis client in manifest | `ioredis`/`redis`/`redis-py` in deps | "Add Redis to docker-compose.yml?" |
| MongoDB driver in manifest | `mongodb`/`pymongo` in deps | "Add MongoDB to docker-compose.yml?" |
| RabbitMQ client in manifest | `amqplib`/`pika`/`bunny` in deps | "Add RabbitMQ to docker-compose.yml?" |
| Kafka client in manifest | `kafkajs`/`confluent-kafka` in deps | "Add Kafka to docker-compose.yml?" |
| SQLite file path in config | `sqlite`/`sqlite3` in connection strings | No compose container (embedded file) |
| Laravel framework detected | `Framework::Laravel` | "Generate queue worker + scheduler services?" |
| Prisma schema present | `prisma/schema.prisma` exists | "Run automatic migrations on startup?" |
| Dockerfile already exists | `Dockerfile` in project root | "Overwrite existing Dockerfile?" |
| Non-standard port detected | Port other than 80/443/3000/8080 | "Confirm exposed port {port}?" |
| Monorepo with >5 services | Service count exceeds threshold | "Generate individual or combined Dockerfiles?" |

**Non-interactive mode (`-y` / `--json`):** All questions default to "yes" for additive actions (add compose services) and "no" for destructive actions (overwrite existing files). The `--json` flag outputs the full question/answer set as structured JSON for programmatic consumers.

---

## 3. Target Codebase Architecture & File Tree

```
dockgen/
├── AGENTS.md                              # This file — single source of truth
├── Cargo.toml                             # Dependencies & metadata
├── Cargo.lock                             # Pinned dependency versions
├── .gitignore
├── README.md
│
├── src/
│   ├── main.rs                            # CLI entry point — clap parsing, anyhow
│   │                                      #   error handler, pipeline orchestration,
│   │                                      #   terminal banner & summary display
│   ├── cli.rs                             # CLI argument definitions (clap derive),
│   │                                      #   override parsers, GenerationConfig builder
│   ├── models.rs                          # Core domain types — enums & structs with
│   │                                      #   Display, Serialize, Deserialize, Clone
│   │
│   ├── detector/                          # ── Phase 1: Discovery ──
│   │   ├── mod.rs                         #   analyze_full_project() orchestrator,
│   │   │                                  #   Service construction, warning collection
│   │   ├── structure.rs                   #   Workspace & monorepo topology detection,
│   │   │                                  #   service candidate enumeration
│   │   ├── language.rs                    #   Language detection — manifest-first,
│   │   │                                  #   heuristic fallback, package manager
│   │   └── framework.rs                   #   Framework detection — dependency scan,
│   │                                      #   config files, entry-point heuristics
│   │
│   ├── analyzer/                          # ── Manifest & Version Analysis ──
│   │   ├── mod.rs                         #   (reserved for future analyzer orchestration)
│   │   ├── dependencies.rs                #   ManifestInfo struct + 11 parsers
│   │   │                                  #   (Cargo, package.json, go.mod, pom.xml,
│   │   │                                  #    build.gradle, requirements.txt,
│   │   │                                  #    pyproject.toml, Pipfile, composer.json,
│   │   │                                  #    .csproj, Gemfile)
│   │   └── version.rs                     #   Runtime version extraction for 8
│   │                                      #   ecosystems (.nvmrc, go.mod, Cargo.toml,
│   │                                      #   pom.xml, etc.)
│   │
│   ├── generator/                         # ── Code Generation ──
│   │   ├── mod.rs                         #   generate_all_files() orchestrator,
│   │   │                                  #   write_generated_files(), dry-run display
│   │   ├── dockerfile.rs                  #   Dockerfile generation — context building,
│   │   │                                  #   binary resolution, Python short version,
│   │   │                                  #   PM run prefix, template selection
│   │   ├── dockerignore.rs                #   .dockerignore generation — polyglot root
│   │   │                                  #   synthesis, per-language templates
│   │   └── compose.rs                     #   docker-compose.yml generation — slugified
│   │                                      #   names, dockerfile_path, port mapping,
│   │                                      #   env vars, forward-slash normalization
│   │
│   ├── interactive/                       # ── Phase 2: Interactive Wizard ──
│   │   ├── mod.rs                         #   (reserved — interactive orchestrator)
│   │   ├── questions.rs                   #   (reserved — question definitions &
│   │   │                                  #    trigger-to-question mappings)
│   │   └── prompts.rs                     #   (reserved — terminal prompt rendering)
│   │
│   └── templates/                         # ── Embedded Template Engine ──
│       └── mod.rs                         #   RustEmbed loader, Tera engine init,
│                                          #   template resolution by language/framework,
│                                          #   test suite (20+ render tests)
│
├── templates/                             # ── Embedded Tera Templates ──
│   ├── dockerfile/                        #   Dockerfile templates per ecosystem
│   │   ├── generic.tera                   #   Ultimate fallback
│   │   ├── node/
│   │   │   ├── nextjs.tera                #   Standalone output, nextjs:nodejs user
│   │   │   ├── nuxt.tera                  #   .output/server standalone
│   │   │   ├── nestjs.tera                #   dist/main.js, nestjs user
│   │   │   ├── sveltekit.tera             #   build/index.js
│   │   │   ├── remix.tera                 #   remix-serve
│   │   │   ├── astro.tera                 #   dist/server/entry.mjs
│   │   │   └── generic.tera               #   Node.js generic fallback
│   │   ├── python/
│   │   │   ├── fastapi.tera               #   uvicorn, py_short_version
│   │   │   ├── django.tera                #   gunicorn + DJANGO_SETTINGS_MODULE
│   │   │   └── generic.tera               #   Dynamic CMD by framework
│   │   ├── go/
│   │   │   ├── generic.tera               #   Distroless + alpine variants
│   │   │   └── gin.tera                   #   Distroless + fallback entrypoint
│   │   ├── rust/
│   │   │   ├── generic.tera               #   cargo-chef caching, 3-stage build
│   │   │   └── axum.tera                  #   --bin {{ bin_name }} build
│   │   ├── java/
│   │   │   └── springboot.tera            #   Maven/Gradle conditional
│   │   ├── php/
│   │   │   ├── laravel.tera               #   Artisan migrate, queue worker
│   │   │   └── generic.tera               #   PHP generic fallback
│   │   ├── dotnet/
│   │   │   └── aspnetcore.tera            #   SDK → ASP.NET runtime
│   │   └── ruby/
│   │       ├── rails.tera                 #   Rails-specific bundling
│   │       └── generic.tera               #   Ruby generic fallback
│   │
│   ├── dockerignore/                      #   .dockerignore templates per language
│   │   ├── node.tera
│   │   ├── python.tera
│   │   ├── go.tera
│   │   ├── rust.tera
│   │   ├── java.tera
│   │   ├── php.tera
│   │   ├── dotnet.tera
│   │   ├── ruby.tera
│   │   └── generic.tera
│   │
│   └── compose/
│       └── docker-compose.yml.tera        #   Multi-service compose with
│                                          #   dynamic dockerfile_path
│
└── tests/
    └── cli_tests.rs                       #   10 integration tests — --help,
                                           #   --version, --dry-run, --json,
                                           #   --compose, combined flags,
                                           #   language override, mock projects
```

### Module Dependency Graph

```
main.rs
  └─ cli.rs            (clap derive, GenerationConfig)
  └─ detector/
  │    ├─ structure.rs  (workspace topology)
  │    ├─ language.rs   (language + PM detection)
  │    ├─ framework.rs  (framework detection)
  │    └─ mod.rs        (orchestrator → Service, ProjectAnalysis)
  └─ analyzer/
  │    ├─ dependencies.rs (ManifestInfo parsers)
  │    └─ version.rs      (runtime version extraction)
  └─ generator/
  │    ├─ dockerfile.rs   (Dockerfile generation)
  │    ├─ dockerignore.rs (.dockerignore generation)
  │    ├─ compose.rs      (docker-compose.yml generation)
  │    └─ mod.rs          (orchestrator → GeneratedFile, write)
  └─ templates/mod.rs     (RustEmbed + Tera engine)
  └─ models.rs            (shared domain types)
```

---

## 4. Infrastructure & Stack Detection Matrix

DockGen detects the following infrastructure dependencies from multiple indicator sources and emits corresponding `docker-compose.yml` service blocks when `--compose` is enabled.

### PostgreSQL

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `DATABASE_URL` contains `postgres://` or `postgresql://` |
| npm dependencies | `pg`, `sequelize`, `typeorm`, `prisma`, `knex`, `drizzle-orm` |
| Python dependencies | `psycopg2`, `asyncpg`, `sqlalchemy`, `django`, `databases` |
| Go dependencies | `pgx`, `pq`, `gorm` with postgres driver |
| Config files | `prisma/schema.prisma` with postgres datasource |
| Connection strings | Any URL matching `postgres(ql)?://` |

**Compose spec:**
```yaml
postgres:
  image: postgres:16-alpine
  ports: ["5432:5432"]
  environment:
    POSTGRES_USER: ${POSTGRES_USER:-app}
    POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-secret}
    POSTGRES_DB: ${POSTGRES_DB:-app}
  volumes:
    - pgdata:/var/lib/postgresql/data
```

### MySQL / MariaDB

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `DATABASE_URL` contains `mysql://` or `mariadb://` |
| npm dependencies | `mysql2`, `mysql`, `sequelize`, `typeorm`, `knex` |
| Python dependencies | `pymysql`, `mysqlclient`, `sqlalchemy` with mysql driver |
| Go dependencies | `go-sql-driver/mysql`, `gorm` with mysql driver |
| Config files | `prisma/schema.prisma` with mysql datasource |

**Compose spec:**
```yaml
mysql:
  image: mysql:8.0
  ports: ["3306:3306"]
  environment:
    MYSQL_ROOT_PASSWORD: ${MYSQL_ROOT_PASSWORD:-secret}
    MYSQL_DATABASE: ${MYSQL_DATABASE:-app}
    MYSQL_USER: ${MYSQL_USER:-app}
    MYSQL_PASSWORD: ${MYSQL_PASSWORD:-secret}
  volumes:
    - mysqldata:/var/lib/mysql
```

### Redis

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `REDIS_URL`, `REDIS_HOST`, `UPSTASH_REDIS_REST_URL` |
| npm dependencies | `ioredis`, `redis`, `bull`, `bullmq`, `cache-manager` |
| Python dependencies | `redis`, `aioredis`, `django-redis`, `celery[redis]` |
| Go dependencies | `go-redis`, `redigo` |
| Config files | `cache.store` = `redis` in Laravel `.env` |

**Compose spec:**
```yaml
redis:
  image: redis:7-alpine
  ports: ["6379:6379"]
  command: redis-server --appendonly yes
  volumes:
    - redisdata:/data
```

### MongoDB

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `MONGODB_URI`, `MONGO_URL`, `DATABASE_URL` contains `mongodb://` |
| npm dependencies | `mongodb`, `mongoose` |
| Python dependencies | `pymongo`, `motor`, `mongoengine` |
| Go dependencies | `mongo-go-driver` |

**Compose spec:**
```yaml
mongo:
  image: mongo:7
  ports: ["27017:27017"]
  environment:
    MONGO_INITDB_ROOT_USERNAME: ${MONGO_USER:-app}
    MONGO_INITDB_ROOT_PASSWORD: ${MONGO_PASSWORD:-secret}
  volumes:
    - mongodata:/data/db
```

### RabbitMQ

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `AMQP_URL`, `RABBITMQ_URL`, `AMQP_HOST` |
| npm dependencies | `amqplib`, `rabbitmq`, `rascal` |
| Python dependencies | `pika`, `celery[rabbitmq]`, `aio-pika` |
| Go dependencies | `amqp091-go`, `rabbitmq` |

**Compose spec:**
```yaml
rabbitmq:
  image: rabbitmq:3-management-alpine
  ports:
    - "5672:5672"
    - "15672:15672"
  environment:
    RABBITMQ_DEFAULT_USER: ${RABBITMQ_USER:-app}
    RABBITMQ_DEFAULT_PASS: ${RABBITMQ_PASS:-guest}
```

### Apache Kafka

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `KAFKA_BROKERS`, `KAFKA_BOOTSTRAP_SERVERS` |
| npm dependencies | `kafkajs`, `kafka-node`, `@confluentinc/kafka-javascript` |
| Python dependencies | `confluent-kafka`, `aiokafka`, `kafka-python` |
| Go dependencies | `confluent-kafka-go`, `sarama`, `segmentio/kafka-go` |

**Compose spec:**
```yaml
kafka:
  image: confluentinc/cp-kafka:latest
  ports: ["9092:9092"]
  environment:
    KAFKA_NODE_ID: 1
    KAFKA_PROCESS_ROLES: broker,controller
    KAFKA_LISTENERS: PLAINTEXT://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093
    KAFKA_CONTROLLER_QUORUM_VOTERS: 1@kafka:9093
    KAFKA_CONTROLLER_LISTENER_NAMES: CONTROLLER
    CLUSTER_ID: ${KAFKA_CLUSTER_ID:-MkU3OEVBNTcwNTJENDM2Qk}
  volumes:
    - kafkadata:/var/lib/kafka/data
```

### SQLite

| Indicator Source | Detection Pattern |
|-----------------|-------------------|
| Environment variables | `DATABASE_URL` contains `sqlite://` or `sqlite:///` |
| npm dependencies | `better-sqlite3`, `sqlite3`, `sql.js` |
| Python dependencies | `sqlite3` (stdlib), `aiosqlite` |
| Config files | Prisma with `provider = "sqlite"` |
| File patterns | `*.db`, `*.sqlite`, `*.sqlite3` in project root |

**Compose spec:** None — SQLite is an embedded file-based database. No separate container is generated. A volume mount may be suggested for data persistence.

---

## 5. Adaptive Question Rules (Phase 2)

The interactive wizard (`-i`) uses Phase 1 analysis results to generate contextually relevant questions. Questions are only asked when their trigger condition is met.

### Trigger-to-Question Mapping

| Trigger Condition | Question | Default |
|-------------------|----------|---------|
| `postgres` driver/URL detected | "Add PostgreSQL service to docker-compose.yml?" | Yes |
| `mysql`/`mariadb` driver detected | "Add MySQL service to docker-compose.yml?" | Yes |
| `redis` client detected | "Add Redis service to docker-compose.yml?" | Yes |
| `mongodb` driver detected | "Add MongoDB service to docker-compose.yml?" | Yes |
| `rabbitmq` client detected | "Add RabbitMQ service to docker-compose.yml?" | Yes |
| `kafka` client detected | "Add Kafka service to docker-compose.yml?" | Yes |
| `sqlite` detected | "SQLite detected (embedded). No compose container needed." | Info only |
| `Framework::Laravel` | "Generate queue worker service?" | Yes |
| `Framework::Laravel` | "Generate scheduler (cron) service?" | Yes |
| `prisma/schema.prisma` exists | "Run `prisma migrate deploy` on startup?" | Yes |
| Existing `Dockerfile` in root | "Overwrite existing Dockerfile?" | No |
| `Framework::SpringBoot` + Gradle | "Use Gradle wrapper in build?" | Yes (detected) |
| Multiple services, no `--force-single` | "Generate individual Dockerfiles or named root files?" | Individual |
| Port ≠ standard (80/443/3000/8080) | "Confirm exposed port {port}?" | Yes |
| Monorepo with >5 services | "Generate compose file for all services?" | Yes |

### Non-Interactive Defaults (`-y` / `--json`)

When `-y` or `--json` is active, all questions auto-resolve:

- **Additive actions** (add compose service, add migration step): Default **Yes**.
- **Destructive actions** (overwrite existing file): Default **No**.
- **Ambiguous actions** (port confirmation): Default **Yes** (trust Phase 1 detection).

The `--json` flag outputs a `question_log` array alongside `analysis` and `files`, recording every auto-resolved question with its trigger, text, and resolved answer.

---

## 6. Container Hardening & Determinism Invariants

Every generated `Dockerfile` must satisfy ALL of the following invariants without exception.

### Non-Root User Compliance

```
RUN addgroup --system --gid 1001 appgroup && \
    adduser  --system --uid 1001 --ingroup appgroup --shell /bin/sh --create-home appuser
USER appuser
```

| Base Image | Alpine Command | Slim/Debian Command |
|-----------|---------------|---------------------|
| Create group | `addgroup -g 1001 appgroup` | `groupadd --gid 1001 appgroup` |
| Create user | `adduser -u 1001 -G appgroup -s /bin/sh -D appuser` | `useradd --uid 1001 --gid appgroup --shell /bin/sh --create-home appuser` |
| Switch user | `USER appuser` | `USER appuser` |

**Exceptions:** Next.js uses `nextjs:nodejs` (UID 1001). NestJS uses `nestjs:nodejs`. Distroless images use `nonroot:nonroot` (UID 65534).

### Multi-Stage Isolation

Every Dockerfile uses **at minimum 2 stages**:

1. **Builder stage** (`AS builder`) — installs build tools, compiles/bundles the application.
2. **Runner stage** (`AS runner`) — minimal runtime image, copies only compiled artifacts.

Languages with dependency caching add a **deps stage** (`AS deps`) between builder and runner:
- Node.js: `deps` stage for `node_modules` caching.
- Rust: 3-stage with `chef-prep` → `chef-cook` → `builder` for cargo-chef caching.

### Forward-Slash Path Normalization

All generated file paths use forward slashes regardless of host OS:
```rust
pub fn to_slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_string()
}
```

Applied in:
- `dockerfile.rs` — monorepo service paths.
- `compose.rs` — build context paths and `dockerfile_path`.

### Zero Shell Piping in COPY

All `COPY` directives use explicit file/directory arguments. No shell piping (`COPY script.sh | sh`). Template variables are interpolated directly into `COPY` source paths.

### Deterministic Output

Given identical input, DockGen produces **byte-identical** output every time.

| Mechanism | Implementation |
|-----------|---------------|
| Sorted services | `analysis.services` sorted by `name` before iteration |
| Sorted env vars | `service.env_vars` sorted by key before emission |
| Sorted generated files | `files.sort_by(\|a, b\| a.relative_path.cmp(&b.relative_path))` |
| Sorted dependencies | Manifest parsers sort dependency lists |
| Stable template rendering | Tera templates use only deterministic variables (no timestamps, no randomness) |
| Deterministic compose | Service entries built in iteration order; sorted by slug name |

### Health Checks

Templates do NOT emit `HEALTHCHECK` instructions by default — this is the user's responsibility. However, compose templates can include health checks when infrastructure services are detected (Postgres, Redis, etc.) via Phase 2 prompts.

### No Secrets in Build Args

- Never emit `ENV` lines containing tokens, passwords, or API keys.
- Never emit `ARG` lines with secret defaults.
- Environment variable placeholders use `${VAR:-default}` syntax in compose templates only, with non-sensitive defaults.

---

## 7. Rust Engineering & Coding Standards

### Language & Toolchain

| Property | Value |
|----------|-------|
| Edition | Rust 2021 |
| MSRV | 1.75.0 |
| Clippy | `cargo clippy --all-targets -- -D warnings` — zero warnings |
| Formatting | `cargo fmt` — default settings, all checked-in code |
| Binary name | `dockgen` |
| License | MIT OR Apache-2.0 |

### Error Handling Hierarchy

```
┌─────────────────────────────────────┐
│  main.rs  (anyhow)                  │  Top-level error reporting
│  → fn run() -> anyhow::Result<()>   │  Ergonomic `?` propagation
├─────────────────────────────────────┤
│  generator/  (anyhow)               │  File generation errors
│  analyzer/   (anyhow)               │  Manifest parsing errors
├─────────────────────────────────────┤
│  detector/   (anyhow)               │  Detection errors
│  models.rs   (thiserror)            │  Domain error types
│  templates/  (tera::Error)          │  Template rendering errors
└─────────────────────────────────────┘
```

**Rules:**

1. **Zero `unwrap()` / `panic!()` in non-test code.** Every fallible operation uses explicit error propagation.
2. `thiserror` for library/domain error types — implements `Display` + `std::error::Error` via derive.
3. `anyhow` at the CLI boundary and generation pipeline — ergonomic `?` with context strings.
4. Early-return with `?` operator; no nested `match` arms for error handling.
5. All error messages must be actionable: "failed to render Dockerfile for service 'api'" not "render failed".

### Path Handling

- All file system paths use `std::path::PathBuf` and `std::path::Path` exclusively.
- No hardcoded path separators (`/` or `\`). Use `Path::join`, `Path::components`, and platform-agnostic APIs.
- Canonicalize paths only when strictly necessary; prefer relative paths in output.
- Cross-platform normalization via `to_slash_path()` for all generated Docker paths.

### Dependency Policy

| Category | Crates |
|----------|--------|
| CLI | `clap` v4 (derive, env, cargo features) |
| Serialization | `serde` v1 (derive), `serde_json` v1, `toml` v0.8 |
| Templating | `tera` v1.19 |
| Embedded assets | `rust-embed` v8 (interpolate-folder-path) |
| Filesystem | `walkdir` v2.4, `ignore` v0.4 |
| Error handling | `anyhow` v1.0, `thiserror` v1.0 |
| Terminal | `colored` v2.1 |
| Logging | `tracing` v0.1, `tracing-subscriber` v0.3 (fmt, env-filter) |
| Regex | `regex` v1.10 |
| Dev-only | `tempfile` v3, `assert_cmd` v2, `predicates` v3 |

**Rules:**
- Pin minimum versions in `Cargo.toml`; let `Cargo.lock` handle exact resolution.
- Prefer well-maintained, widely-used crates. No obscure or unmaintained dependencies.
- Feature-gate optional functionality where possible.

### Testing Conventions

| Type | Location | Pattern |
|------|----------|---------|
| Unit tests | `#[cfg(test)] mod tests` blocks within each module | `make_service()` helpers, `tera_engine()` fixture |
| Integration tests | `tests/cli_tests.rs` | `assert_cmd` + `predicates`, tempdir fixtures |
| Template render tests | `src/templates/mod.rs::tests` | `tera_render()` helper with context builder |
| Determinism tests | Multiple modules | Assert exact string/byte equality on duplicate runs |

**Test count:** 328 unit tests + 10 integration tests = **338 total** (all passing).

**Test rules:**
- All public API functions must have at least one unit test.
- Test deterministic output: assert exact string equality, not substring matches (where possible).
- Use `tempfile` crate for filesystem tests; clean up after each test.
- Template tests must provide ALL required context variables to catch missing-variable regressions.

### Code Organization Principles

1. **Single-pass detection.** Language/framework detection scans the project once. No redundant directory walks.
2. **Model-first.** All logic operates on `models.rs` types. No raw strings or ad-hoc JSON in business logic.
3. **Templates are assets.** Tera templates live in `templates/` and are embedded at compile time via `rust-embed`. Never read from disk at runtime.
4. **Composable generation.** Each `Service` produces an independent `GeneratedFile` set. Compose files (if requested) are assembled from service outputs.
5. **No global mutable state.** All state flows through function arguments. The CLI parses args into a config, passes it through analysis → generation → emission.
6. **Fail fast, report clearly.** On detection failure or ambiguous project structure, emit actionable warnings (collected in `ProjectAnalysis::warnings`) rather than silently guessing.

### Documentation

- Every public type and function must have a `///` doc comment.
- `AGENTS.md` is the single source of truth for architecture decisions.
- Internal module-level comments use `// ---` section headers for visual grouping.

---

## 8. Pipeline Execution Order

```
┌─────────────────────────────────────────────────────────────────────┐
│  Step 1: CLI Parsing                                                │
│  Cli::parse() → Cli struct                                          │
├─────────────────────────────────────────────────────────────────────┤
│  Step 2: Path Resolution                                            │
│  cli.get_target_path() → canonical PathBuf                          │
├─────────────────────────────────────────────────────────────────────┤
│  Step 3: Phase 1 — Analysis                                         │
│  detector::analyze_full_project(path, lang?, fw?, services?)        │
│    ├─ structure::detect_workspace() → WorkspaceStructure            │
│    ├─ language::detect_language() → (Language, PackageManager)      │
│    ├─ framework::detect_framework() → Framework                     │
│    ├─ analyzer::dependencies::parse_manifest() → ManifestInfo       │
│    ├─ analyzer::version::extract_version() → Option<String>         │
│    └─ Service { name, path, package_name, language, framework, ... }│
│  Output: ProjectAnalysis { root_path, services, warnings, ... }     │
├─────────────────────────────────────────────────────────────────────┤
│  Step 4: JSON Mode (early exit)                                     │
│  if --json → serialize analysis + files → stdout → exit             │
├─────────────────────────────────────────────────────────────────────┤
│  Step 5: Terminal Display                                            │
│  print_banner() → print_analysis_report(analysis)                   │
├─────────────────────────────────────────────────────────────────────┤
│  Step 6: Generation                                                  │
│  generator::generate_all_files(analysis, config)                    │
│    ├─ dockerfile::generate_dockerfiles() → Vec<GeneratedFile>       │
│    ├─ dockerignore::generate_dockerignores() → Vec<GeneratedFile>   │
│    └─ compose::generate_docker_compose() → Option<GeneratedFile>    │
│  Sort all files by relative_path (deterministic ordering).          │
├─────────────────────────────────────────────────────────────────────┤
│  Step 7: Emission                                                    │
│  generator::write_generated_files(files, output_dir, dry_run)       │
│    ├─ dry_run → print_dry_run() → stdout                            │
│    └─ !dry_run → create_dir_all() + fs::write() per file            │
├─────────────────────────────────────────────────────────────────────┤
│  Step 8: Summary                                                     │
│  print_summary(files, config, target_path)                          │
│  Suggests docker build or docker compose commands.                   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 9. Template Resolution Rules

Templates are resolved by language and framework via `resolve_dockerfile_template()`:

| Language | Framework | Template Path |
|----------|-----------|---------------|
| Node.js | NextJs | `dockerfile/node/nextjs.tera` |
| Node.js | Nuxt | `dockerfile/node/nuxt.tera` |
| Node.js | NestJs | `dockerfile/node/nestjs.tera` |
| Node.js | SvelteKit | `dockerfile/node/sveltekit.tera` |
| Node.js | Remix | `dockerfile/node/remix.tera` |
| Node.js | Astro | `dockerfile/node/astro.tera` |
| Node.js | * | `dockerfile/node/generic.tera` |
| Python | FastApi | `dockerfile/python/fastapi.tera` |
| Python | Django | `dockerfile/python/django.tera` |
| Python | * | `dockerfile/python/generic.tera` |
| Go | Gin | `dockerfile/go/gin.tera` |
| Go | * | `dockerfile/go/generic.tera` |
| Rust | Axum | `dockerfile/rust/axum.tera` |
| Rust | * | `dockerfile/rust/generic.tera` |
| Java | SpringBoot | `dockerfile/java/springboot.tera` |
| Java | * | `dockerfile/java/springboot.tera` |
| PHP | Laravel | `dockerfile/php/laravel.tera` |
| PHP | * | `dockerfile/php/generic.tera` |
| .NET | * | `dockerfile/dotnet/aspnetcore.tera` |
| Ruby | Rails | `dockerfile/ruby/rails.tera` |
| Ruby | * | `dockerfile/ruby/generic.tera` |
| * | * | `dockerfile/generic.tera` |

### Context Variables Injected by `build_dockerfile_context()`

| Variable | Type | Source |
|----------|------|--------|
| `port` | `u16` | Override → detected → 8080 |
| `runtime_version` | `String` | Detected → default per language |
| `base_image_variant` | `String` | `--base` flag → `"alpine"` |
| `build_command` | `Option<String>` | `Service.build_command` |
| `start_command` | `Option<String>` | `Service.start_command` |
| `package_manager` | `String` | `PackageManager.to_string()` |
| `language` | `String` | `Language.to_string()` |
| `framework` | `String` | `Framework.to_string()` |
| `bin_name` | `String` | `package_name` → entrypoint stem → `name` |
| `assembly_name` | `String` | Same as `bin_name` (for .NET) |
| `py_short_version` | `String` | `"3.11"` from `"3.11.9"` (Rust-computed) |
| `pm_run_prefix` | `String` | `"npm run"` / `"pnpm"` / `"yarn"` / `"bun run"` |
| `build_tool` | `String` | `"gradle"` or `"maven"` |
| `has_frontend_assets` | `bool` | Node.js `package.json` in non-Node service |
| `node_pm` | `String` | `"pnpm"` / `"yarn"` / `"bun"` / `"npm"` |
| `entrypoint_file` | `String` | Framework-specific entry file |
| `entrypoint_dir` | `String` | Framework-specific entry directory |
| `env_vars` | `Vec<{key, value}>` | Sorted by key |

### Binary Name Resolution (`resolve_bin_name`)

```
service.package_name  →  service.entrypoint (stem)  →  service.name
     ↓                         ↓                           ↓
 "my-api"              "cmd/server" → "server"         "backend"
```

Priority: manifest `package_name` > entrypoint file stem > directory name.

### PM Run Prefix (`compute_pm_run_prefix`)

| Package Manager | Prefix |
|----------------|--------|
| npm, Unknown | `npm run` |
| pnpm | `pnpm` |
| yarn | `yarn` |
| bun | `bun run` |

Used in templates: `{{ pm_run_prefix }} build` → `npm run build`.

---

## 10. Future Roadmap (Phase 2 Modules)

The following modules are reserved in the file tree but not yet implemented:

| Module | Purpose |
|--------|---------|
| `src/interactive/mod.rs` | Interactive wizard orchestrator |
| `src/interactive/questions.rs` | Question definitions + trigger-to-question mapping |
| `src/interactive/prompts.rs` | Terminal prompt rendering (dialoguer/indicatif) |
| `src/analyzer/mod.rs` | Analyzer orchestration (currently unused) |

**Phase 2 implementation order:**
1. `interactive/questions.rs` — Define all question types and trigger conditions.
2. `interactive/prompts.rs` — Terminal UI with `dialoguer` for select/confirm prompts.
3. `interactive/mod.rs` — Orchestrate question flow, collect answers, inject into `GenerationConfig`.
4. `src/main.rs` — Add `-i` / `--yes` flag handling between Step 3 (analysis) and Step 6 (generation).
