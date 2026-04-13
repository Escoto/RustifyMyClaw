# Contributing

Contributions are welcome. This guide covers everything from setting up a dev environment to getting a PR merged.

## Good first issues

Issues labeled `good first issue` are scoped to a single module and don't require deep knowledge of the full pipeline. Start there if you're new to the codebase.

For larger changes, open an issue first to discuss the approach before writing code.

## Development setup

Requirements:
- Rust stable (edition 2021). Install via [rustup](https://rustup.rs/).
- No nightly features, no system packages beyond a C linker.

```bash
git clone https://github.com/Escoto/RustifyMyClaw.git
cd RustifyMyClaw
cargo build
cargo test              # should show 125 tests passing
cargo clippy -- -D warnings
cargo fmt --check
```

If all four commands pass, your environment is ready.

## Branch conventions

Branch from `main`. Use one of these prefixes:

| Prefix | Use for |
|--------|---------|
| `feat/` | New functionality |
| `fix/` | Bug fixes |
| `docs/` | Documentation only |
| `refactor/` | Code changes with no behavior change |

Examples: `feat/discord-channel`, `fix/session-not-reset`, `docs/configuration-reference`.

No direct pushes to `main`.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format. Imperative mood. One logical change per commit.

Good:
```
feat: add Discord channel provider
fix: reset session on /new when workspace is unchanged
docs: add per-channel output override examples
```

Bad:
```
added stuff
fix things
WIP
```

Don't mix a refactor with a feature in the same commit.

## Pull request process

1. Open a PR against `main`.
2. Fill in the PR template — description and checklist.
3. Link the related issue if one exists (`Closes #123`).
4. All CI checks must pass: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
5. PRs are squash-merged. The PR title becomes the commit message on `main`.
6. Reviews are best-effort — no guaranteed turnaround for a young project.

## Testing expectations

Every new feature needs at least one test. Every bug fix needs a regression test that fails before the fix and passes after.

- Unit tests live in `#[cfg(test)] mod tests` blocks within each source file, using `#[path = "tests/..."]` to keep test files separate.
- Integration tests live in `src/tests/`.
- Use `MockBackend` for executor tests — never invoke a real CLI in unit tests.
- No test may depend on network access, filesystem state outside a temp dir, or wall-clock timing.
- Tests must be deterministic. No random data without seeds.

See the "What to Test Per Module" table in `CLAUDE.md` for module-specific coverage requirements.

## Code style

See `CLAUDE.md` for the full Rust style guide. The non-negotiables:

- `cargo fmt` before every commit.
- `cargo clippy -- -D warnings` must pass clean.
- No `unwrap()` or `expect()` in library code. Use `?` with `anyhow::Result`.
- No `println!`. Use `tracing::{info, warn, error, debug, trace}`.
- No `.clone()` to satisfy the borrow checker. Redesign ownership or use `Arc`.
- No direct pushes or force-pushes to shared branches.

## Architecture decisions

Changes that affect module boundaries, add a new backend or channel, or touch the core pipeline need a discussion issue before a PR. Don't surprise maintainers with a 500-line structural change.

See `docs/architecture.md` for the system design and extension points.

## Release process

Releases are maintainer-only via `workflow_dispatch` on `main`:

1. Update `Cargo.toml` version to match the intended release.
2. Trigger the **Release** workflow from GitHub Actions, selecting the bump type (patch/minor/major).
3. The workflow validates the version, builds cross-platform artifacts, creates a GitHub Release with checksums, then automatically publishes to **crates.io** and **Chocolatey** in parallel.

Secrets (`CARGO_REGISTRY_TOKEN`, `CHOCO_API_KEY`) are scoped to the `release` environment. Contributors don't need access to these — the pipeline handles everything after merge.

## Code of Conduct

This project follows the [Contributor Covenant v2.1](CODE_OF_CONDUCT.md).
