# Contributing to PTE

Thanks for your interest in contributing to Pixelated's Tibia Editor!

## Branch Model

| Branch | Purpose |
|--------|---------|
| `master` | Stable release branch — never push directly |
| `staging` | Integration branch — all PRs target here |
| `feat/*` | Feature branches |
| `fix/*` | Bug fix branches |
| `refactor/*` | Refactoring branches |
| `docs/*` | Documentation-only changes |
| `chore/*` | Build/CI/tooling changes |

### Workflow

1. Create a branch from `staging` using the naming convention above
2. Make your changes
3. Open a PR targeting `staging`
4. CI runs clippy, formatting, build, and tests
5. After review and merge, changes auto-promote to `master` via release PRs

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add house brush tool
fix: correct tile rendering at z-level 0
refactor: simplify brush palette layout
docs: update README with new shortcuts
chore: update egui to 0.30
perf: batch sprite sheet uploads
feat!: change OTBM save format (BREAKING)
```

The commit type determines automatic version bumping:
- `feat!:` or `BREAKING CHANGE` → **major** version bump
- `feat:` → **minor** version bump
- `fix:`, `refactor:`, `perf:` → **patch** version bump

## Development Setup

### Prerequisites
- Rust 1.75+ via [rustup](https://rustup.rs)
- Windows with MSVC build tools (Visual Studio or Build Tools)

### Building
```bash
cargo build --release
```

### Running Checks
```bash
# Format check
cargo fmt --check

# Lint
cargo clippy --workspace -- -W clippy::all

# Tests
cargo test --workspace
```

## Code Style

- Follow standard Rust formatting (`cargo fmt`)
- All clippy warnings should be resolved
- Prefer explicit error handling with `anyhow::Result` in application code
- Use `thiserror` for library crate errors
- Keep modules focused — one panel/feature per file in the editor crate

## Architecture Notes

- **Immediate-mode UI** — egui redraws every frame, so avoid allocations in hot render paths
- **State lives in `EditorState`** — all mutable state is centralized, passed as `&mut` to panel functions
- **Crates are libraries** — `assets`, `appearances`, `otbm`, and `spr_dat` are pure libraries with no GUI dependency
- **Background loading** — heavy I/O (asset loading, project creation) runs on background threads with channel-based result passing

## Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR
- Include a clear description of what changed and why
- Add screenshots for UI changes
- Ensure CI passes before requesting review
- Squash-merge is the default merge strategy
