# dockgen

[![crates.io](https://img.shields.io/crates/v/dockgen.svg)](https://crates.io/crates/dockgen)
[![CI](https://github.com/dockgen/dockgen/actions/workflows/ci.yml/badge.svg)](https://github.com/dockgen/dockgen/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust 2021](https://img.shields.io/badge/rust-2021-orange.svg)](https://rust-lang.org)

**Smart minimal Dockerfile generator — languages, frameworks & multi-service projects.**

DockGen scans a project directory, detects languages, frameworks, and package managers, analyses service boundaries (monorepo-aware), and emits production-grade, security-hardened `Dockerfile`s and `.dockerignore` files with zero manual intervention.

---

## Key Features

- **30+ Framework Support** — Next.js, Nuxt, NestJS, FastAPI, Django, Flask, Gin, Echo, Axum, Actix Web, Spring Boot, Laravel, Rails, ASP.NET Core, and more.
- **Monorepo Intelligence** — Automatic detection of Turborepo, pnpm workspaces, Nx, Lerna, Cargo workspaces, and Go modules. Per-service Dockerfiles with correct build contexts.
- **Secure by Default** — Every generated Dockerfile uses non-root users (`USER appuser`), multi-stage builds, and minimal base images (Alpine/Slim).
- **Cache-Efficient Builds** — Manifests and lockfiles are copied first, dependencies installed, then source code layered on top.
- **Deterministic Output** — Same input always produces byte-identical output. Sorted services, sorted files.
- **Zero Runtime Dependencies** — Single static binary. No Node, Python, or Docker required to generate files.
- **Docker Compose Generation** — Optionally emit a `docker-compose.yml` with correct port mappings and environment variables.
- **JSON Output Mode** — Machine-readable analysis and generation results for CI/CD integration.

---

## Supported Languages & Frameworks

| Language | Frameworks | Base Images |
|----------|-----------|-------------|
| **Node.js** | Next.js, Nuxt, NestJS, Express, Fastify, Remix, SvelteKit, Astro, Generic | `node:<ver>-alpine` |
| **Python** | FastAPI, Django, Flask, Starlette, Litestar, Generic | `python:<ver>-slim` |
| **Go** | Gin, Echo, Fiber, Chi, Generic | `golang:<ver>-alpine` → `alpine:3.19` |
| **Rust** | Axum, Actix Web, Rocket, Warp, Generic | `rust:<ver>-slim` → `debian:bookworm-slim` |
| **Java** | Spring Boot, Quarkus, Micronaut, Generic | `eclipse-temurin:<ver>-jre-alpine` |
| **PHP** | Laravel, Symfony, Generic | `php:<ver>-fpm-alpine` |
| **.NET** | ASP.NET Core, Generic | `mcr.microsoft.com/dotnet/aspnet:<ver>-alpine` |
| **Ruby** | Rails, Sinatra, Generic | `ruby:<ver>-slim` |
| **Generic** | Any | `alpine:3.19` |

---

## Installation

### From crates.io

```bash
cargo install dockgen
```

### From source

```bash
git clone https://github.com/dockgen/dockgen.git
cd dockgen
cargo install --path .
```

### Pre-built binaries

Download the latest binary from [GitHub Releases](https://github.com/dockgen/dockgen/releases) for your platform.

---

## Usage

### Automatic detection (simplest)

```bash
# Scan current directory and generate files
dockgen

# Scan a specific project
dockgen ./my-project

# Preview without writing to disk
dockgen --dry-run
```

### Language & framework overrides

```bash
# Force Rust + Axum detection
dockgen -l rust -f axum

# Force Python + FastAPI
dockgen --lang python --fw fastapi

# Force Node.js + Next.js
dockgen -l node -f next
```

### Monorepo handling

```bash
# Auto-detect monorepo and generate per-service Dockerfiles
dockgen ./monorepo

# Generate only for specific services
dockgen ./monorepo -s frontend,api

# Force single unified Dockerfile
dockgen ./monorepo --single
```

### Port & base image overrides

```bash
# Override ports for services (in order)
dockgen -p 3000,8080

# Prefer Alpine base images
dockgen -b alpine
```

### Docker Compose generation

```bash
# Generate docker-compose.yml alongside Dockerfiles
dockgen --compose

# Combine with other flags
dockgen ./monorepo --compose -p 3000,8080 --dry-run
```

### JSON output (CI/CD)

```bash
# Get structured JSON output
dockgen --json

# Pipe to jq for filtering
dockgen --json | jq '.analysis.services[] | {name, language, framework}'
```

### Quiet mode

```bash
# Suppress all output except errors
dockgen -q

# Combine with JSON for clean machine output
dockgen --json -q
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          CLI (clap v4)                          │
│  path · lang · fw · services · port · base · compose · dry-run │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Analysis Pipeline                           │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Structure   │→ │   Language   │→ │     Framework        │  │
│  │  Detector    │  │  + PM Det.   │  │  + Version Extract   │  │
│  │  (workspace, │  │  (manifests, │  │  (per-ecosystem      │  │
│  │   monorepo)  │  │   extensions)│  │   detectors)         │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                               │                                 │
│                               ▼                                 │
│                    ┌─────────────────────┐                      │
│                    │  ProjectAnalysis    │                      │
│                    │  (services, warnings│                      │
│                    │   monorepo flag)    │                      │
│                    └─────────┬───────────┘                      │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Generation Pipeline                         │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Dockerfile  │  │ .dockerignore│  │  docker-compose.yml  │  │
│  │  Generator   │  │  Generator   │  │  Generator (opt.)    │  │
│  │  (per-svc,   │  │  (per-svc +  │  │  (services, ports,   │  │
│  │   tera tpl)  │  │   root)      │  │   env vars)          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                               │                                 │
│                               ▼                                 │
│                    ┌─────────────────────┐                      │
│                    │  GeneratedFile(s)   │                      │
│                    │  (sorted, ready)    │                      │
│                    └─────────┬───────────┘                      │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Output Layer                              │
│                                                                 │
│  dry-run: formatted stdout display                              │
│  write:   safe disk write (create_dir_all + fs::write)          │
│  json:    structured JSON to stdout                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## Generated Dockerfile Standards

Every Dockerfile produced by DockGen follows these elite container standards:

- **Multi-stage builds** — Separate builder and runtime stages to minimize image size.
- **Non-root execution** — `USER appuser` or `USER 10001` in every runtime stage.
- **Cache-efficient ordering** — Manifests/lockfiles copied first, dependencies installed, then source code.
- **Minimal base images** — Alpine or Slim variants by default. Distroless available via `--base distroless`.
- **Explicit EXPOSE/ENV** — Port declarations and environment variables clearly stated.
- **No secrets** — Never emits tokens, passwords, or API keys in ENV directives.

---

## CLI Reference

| Flag | Short | Description |
|------|-------|-------------|
| `--lang` | `-l` | Override detected language |
| `--fw` | `-f` | Override detected framework |
| `--services` | `-s` | Filter specific services (comma-separated) |
| `--single` | | Force single unified Dockerfile |
| `--base` | `-b` | Base image variant (alpine/slim/distroless/default) |
| `--compose` | `-c` | Generate docker-compose.yml |
| `--port` | `-p` | Override exposed ports (comma-separated, in order) |
| `--dry-run` | | Preview generated files without writing |
| `--json` | | Output structured JSON |
| `--output-dir` | `-o` | Custom output directory |
| `--verbose` | `-v` | Enable debug tracing |
| `--quiet` | `-q` | Suppress all output except errors |

---

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
