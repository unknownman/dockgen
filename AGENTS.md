# AGENTS.md — DockGen Architecture & Coding Standards

## Project Identity

**dockgen** — Blazing fast Rust CLI for deterministic Dockerfile and `.dockerignore` generation.

DockGen scans a project directory, detects languages/frameworks/package managers, analyses service boundaries (monorepo-aware), and emits production-grade, security-hardened `Dockerfile`s and `.dockerignore` files with zero manual intervention.

---

## Coding Standard

### Language & Toolchain

- **Edition:** Rust 2021.
- **Clippy:** Zero warnings. CI enforces `cargo clippy --all-targets -- -D warnings`.
- **Formatting:** `cargo fmt` with default settings. All checked-in code must be formatted.
- **MSRV:** 1.75.0 (or latest stable LTS).

### Error Handling

- **Zero `unwrap()` / `panic!()` in non-test code.** Every fallible operation must use explicit error propagation.
- Use `thiserror` for library/domain error types. Use `anyhow` at the CLI boundary (binary) for ergonomic top-level error reporting.
- All error types must implement `Display` and `std::error::Error` (via `thiserror` derive).
- Early-return with `?` operator; no nested match arms for error handling.

### Path Handling

- All file system paths use `std::path::PathBuf` and `std::path::Path` exclusively.
- No hardcoded path separators (`/` or `\`). Use `Path::join`, `Path::components`, and platform-agnostic APIs.
- Canonicalize paths only when strictly necessary; prefer relative paths in output.

### Determinism

- **All outputs must be deterministic.** Given the same input, DockGen must produce byte-identical output every time.
- Sort all `Vec` and `BTreeMap`/`HashMap` values before emission. Use sorted iterators, not relying on iteration order.
- Templates rendered via `tera` must produce stable output; avoid random or time-based values.

### Security (Dockerfile Hardening)

- **Non-root user compliance:** Every generated `Dockerfile` must create and switch to a non-root user (`appuser` / `appgroup`).
- **Multi-stage builds:** Use multi-stage patterns to minimize final image size and exclude build tooling.
- **Minimal attack surface:** Prefer `alpine` or `slim` base images. Avoid installing unnecessary packages.
- **No secrets in build args:** Never emit `ENV` lines containing tokens, passwords, or API keys.

### Code Organization

```
src/
├── main.rs            # CLI entry point (clap, anyhow error handler)
├── cli.rs             # CLI argument definitions (clap derive)
├── models.rs          # Core domain types (enums, structs, serde traits)
├── detector.rs        # Language/framework/package-manager detection
├── analyzer.rs        # Project structure analysis & service boundary detection
├── generator.rs       # Dockerfile & dockerignore generation logic
├── templates/         # Embedded tera templates (via rust-embed)
│   ├── Dockerfile.tera
│   └── dockerignore.tera
├── emit.rs            # File emission, dry-run, compose generation
├── error.rs           # Domain error types (thiserror)
└── logging.rs         # Tracing setup
```

### Dependency Policy

- Pin minimum versions in `Cargo.toml`; let `Cargo.lock` handle exact resolution.
- Prefer well-maintained, widely-used crates. No obscure or unmaintained dependencies.
- Feature-gate optional functionality where possible.

### Testing

- Unit tests in `#[cfg(test)] mod tests` blocks within each module.
- Integration tests in `tests/` directory for end-to-end scenarios.
- All public API functions must have at least one unit test.
- Test deterministic output: assert exact string equality, not substring matches.
- Use `tempfile` crate for filesystem tests; clean up after each test.

### Documentation

- Every public type and function must have a `///` doc comment.
- `AGENTS.md` is the single source of truth for architecture decisions.

---

## Architecture Invariants

1. **Single-pass detection.** Language/framework detection scans the project once. No redundant directory walks.
2. **Model-first.** All logic operates on `models.rs` types. No raw strings or ad-hoc JSON in business logic.
3. **Templates are assets.** Tera templates live in `src/templates/` and are embedded at compile time via `rust-embed`. Never read from disk at runtime.
4. **Composable generation.** Each `Service` produces an independent `GeneratedFile` set. Compose files (if requested) are assembled from service outputs.
5. **No global mutable state.** All state flows through function arguments. The CLI parses args into a config, passes it through analysis → generation → emission.
6. **Fail fast, report clearly.** On detection failure or ambiguous project structure, emit actionable warnings (collected in `ProjectAnalysis::warnings`) rather than silently guessing.
