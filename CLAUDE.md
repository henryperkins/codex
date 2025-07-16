# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Codex CLI is a lightweight coding agent from OpenAI that runs locally in your terminal. It uses a hybrid architecture:

- **Rust backend** (`codex-rs/`): Core functionality, sandboxing, execution policies, and performance-critical components
- **TypeScript/React frontend** (`codex-cli/`): CLI interface, chat UI, and build tooling

## Essential Development Commands

### Building and Testing

```bash
# TypeScript/JavaScript (from root directory)
pnpm install        # Install dependencies
pnpm build          # Build the project
pnpm test           # Run TypeScript tests
pnpm typecheck      # Check TypeScript types
pnpm lint           # Run ESLint
pnpm lint:fix       # Fix linting issues
pnpm format:fix     # Format code with Prettier

# Rust (from codex-rs directory)
cargo build         # Build Rust code
cargo test          # Run Rust tests
cargo clippy --tests # Run Rust linter
cargo fmt -- --config imports_granularity=Item  # Format Rust code
```

### Running Codex Locally

```bash
# After building, run from the root directory
node codex-cli/dist/cli.js

# Or link for global use
cd codex-cli && npm link
codex  # Now available globally
```

## Architecture and Key Components

### TypeScript/React Components (`codex-cli/`)

- `src/cli.tsx`: Main CLI entry point
- `src/app.tsx`: Main application component
- `src/components/`: React components for terminal UI
- `src/utils/`: Agent operations, sandboxing, OpenAI client utilities
- `tests/`: Vitest test suite

### Rust Components (`codex-rs/`)

- `core/`: Core Codex functionality library
- `cli/`: Rust CLI binary
- `exec/`: Code execution engine
- `execpolicy/`: Security policies for execution
- `linux-sandbox/`: Linux sandboxing (Landlock/seccomp)
- `mcp-*`: Model Context Protocol implementations
- `file-search/`: File searching capabilities

### Important Configuration Files

- `package.json`: Root monorepo configuration
- `pnpm-workspace.yaml`: pnpm workspace setup
- `codex-rs/Cargo.toml`: Rust workspace configuration
- User config: `~/.codex/config.toml`

## Development Requirements

- Node.js 22+
- Rust 1.88+
- pnpm package manager
- Platform-specific sandboxing support (macOS: Apple Seatbelt, Linux: Landlock/seccomp)

## Critical Development Rules

1. **Always run lint and typecheck** before considering work complete:

   ```bash
   pnpm lint && pnpm typecheck
   cargo clippy --tests
   ```

2. **Include tests** for all new functionality - tests should fail before changes and pass after

3. **Security considerations**:

   - Never modify code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`
   - Network access is disabled by default in sandbox mode
   - Default sandbox mode is read-only

4. **Code formatting**:

   - TypeScript: Use Prettier (via `pnpm format:fix`)
   - Rust: Use `cargo fmt -- --config imports_granularity=Item`

5. **Git commits**: When creating commits, use this format:

   ```
   <commit message>

   🤖 Generated with [Claude Code](https://claude.ai/code)

   Co-Authored-By: Claude <noreply@anthropic.com>
   ```

## Testing Approach

- Run `pnpm test` for TypeScript tests (uses Vitest)
- Run `cargo test` for Rust tests
- Integration tests are in `codex-cli/tests/`
- Unit tests are colocated with source files

## Common Tasks

### Adding a new TypeScript feature

1. Implement in appropriate file under `codex-cli/src/`
2. Add tests in `codex-cli/tests/`
3. Run `pnpm build && pnpm test && pnpm lint && pnpm typecheck`

### Adding a new Rust feature

1. Implement in appropriate crate under `codex-rs/`
2. Add tests in the same crate
3. Run `cargo build && cargo test && cargo clippy --tests`

### Debugging

- Use `RUST_LOG=debug` environment variable for verbose Rust logging
- TypeScript debugging can be done with Node.js inspector

## Project-Specific Context

The project includes an `AGENTS.md` file that can provide additional context for AI agents. This file can be placed at:

- Repository root
- Current working directory
- `~/.codex/AGENTS.md`
