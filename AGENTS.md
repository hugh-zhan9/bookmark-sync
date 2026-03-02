# Repository Guidelines

## Project Structure & Module Organization
This repository has three main parts:
- `bookmark-sync-app/`: Tauri desktop app (React + TypeScript frontend, Rust backend).
- `browser-extension/`: Chrome extension client and native-host installer scripts.
- `docs/`: product and technical design notes, plus change logs.

Within `bookmark-sync-app/`:
- `src/` contains React UI (`App.tsx`, `main.tsx`, styles).
- `src-tauri/src/` contains Rust modules (`db/`, `events/`, `sync/`, `lib.rs` command entrypoints).
- `public/` and `src-tauri/icons/` store static assets and app icons.

## Build, Test, and Development Commands
Run commands from `bookmark-sync-app/` unless noted:
- `npm install`: install frontend and Tauri JS dependencies.
- `npm run dev`: start Vite frontend dev server.
- `npm run tauri dev`: run the desktop app with hot reload.
- `npm run build`: TypeScript compile + Vite production build.
- `npm run tauri build`: produce distributable desktop binaries.
- `cargo check --manifest-path src-tauri/Cargo.toml`: fast Rust compile validation.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust tests (when present).

## Coding Style & Naming Conventions
- TypeScript: 2-space indentation, `camelCase` for vars/functions, `PascalCase` for React components and interfaces.
- Rust: follow `rustfmt` defaults (4 spaces), `snake_case` for functions/modules, `CamelCase` for structs/enums.
- Keep modules focused (`events`, `db`, `sync`) and avoid cross-module leakage of internal helpers.
- Prefer explicit command names for Tauri invokes (e.g., `trigger_sync`, `save_credentials`).

## Testing Guidelines
- Add unit tests for Rust domain logic in `src-tauri/src/**` with `#[cfg(test)] mod tests`.
- For frontend changes, at minimum verify critical flows manually in `npm run tauri dev`:
  add bookmark, search bookmark, save credentials, trigger sync.
- Name tests by behavior (example: `search_returns_non_deleted_bookmarks`).

## Commit & Pull Request Guidelines
- Follow Conventional Commits seen in history (example: `feat: 完成书签同步应用 M1 至 M4 核心里程碑开发`).
- Recommended format: `type(scope): short summary` where `type` is `feat|fix|refactor|docs|test|chore`.
- PRs should include:
  - clear change summary and impacted modules,
  - linked issue/task,
  - local verification steps and results,
  - UI screenshots/GIFs when frontend behavior changes.

## Security & Configuration Tips
- Never commit real tokens, keychain secrets, or private repository URLs.
- Validate extension permissions (`browser-extension/manifest.json`) before release.
- Keep release logic aligned with `.github/workflows/release.yml` and test on at least one local platform before tagging `v*`.
