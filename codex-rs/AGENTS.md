# Repository Guidelines

Welcome! This document outlines day-to-day practices for contributing to the **codex-rs** codebase. Follow these points to keep reviews smooth and builds green.

## Project Structure & Module Organization

- `codex-rs/` — Cargo workspace root; each subfolder beginning with `codex-` is a crate.
- `core/`, `cli/`, `seatbelt/` … — primary crates; every crate owns its own `src/`, `benches/`, `examples/`, and `tests/` directories.
- `tests/` (workspace level) — cross-crate integration tests.
- `assets/` — fixtures, sample data, and non-code resources.

## Build, Test, and Development Commands

- `just build` — compile all crates in the workspace.
- `just fmt` — run `rustfmt` on every crate; must be clean before PR.
- `just fix` — apply `clippy --fix` for lints.
- `cargo test --all-features` — execute the complete test suite.
- `just watch` — incremental `cargo check` on file changes.

## Coding Style & Naming Conventions

- Rust 2021 edition, **4-space** indentation; no tabs.
- Always prefer `format!("… {variable}")` over string concatenation.
- Crate names: `codex-*`; module files: `snake_case.rs`; types & enums: `PascalCase`; variables: `snake_case`.
- All code passes `rustfmt` and `clippy` with **zero** warnings.

## Testing Guidelines

- Unit tests sit next to the code; integration tests in each crate’s `tests/` folder.
- Network access is sandboxed. Skip external calls when `CODEX_SANDBOX_NETWORK_DISABLED=1`.
- Use descriptive test names like `tests::parses_empty_input()`.

## Commit & Pull Request Guidelines

- Follow **Conventional Commits** (`feat:`, `fix:`, `docs:` …).
- Scope prefix (crate or area) is encouraged: `feat(core): add token stream`.
- Squash if a branch has noisy fix-ups before merge.
- A PR must:
  - Pass `just fmt`, `just fix`, and `cargo test`.
  - Link related issues.
  - Explain **what** & **why**; add screenshots/logs if visual or CLI output changes.

## Security & Configuration Tips

- Never edit code paths referencing `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
- Seatbelt sandboxed processes inherit `CODEX_SANDBOX=seatbelt`; tests that spawn child sandboxes should early-return when this is set.

---
Questions or ideas? Open an issue or start a draft PR — collaboration is welcome!

