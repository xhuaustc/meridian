# Repository Guidelines

## Project Structure & Module Organization

Meridian is a Tauri v2 desktop app for managing a local Nginx proxy. The React/TypeScript frontend lives in `src/`: pages in `pages/`, reusable UI in `components/`, Zustand state in `stores/`, IPC wrappers in `lib/`, and Chinese/English strings in `locales/`. Rust backend code lives in `src-tauri/src/`, with Tauri commands, Nginx configuration and process management, certificates, DNS providers, and SQLite storage in separate modules. Keep feature specifications in `specs/`; static assets are in `public/` and `src-tauri/icons/`.

## Build, Test, and Development Commands

- `npm ci`: install dependencies from `package-lock.json`.
- `npm run dev`: run the Vite frontend alone for quick UI work.
- `npm run tauri dev`: run the desktop app with frontend hot reload and Rust compilation.
- `npm run build`: typecheck TypeScript and build frontend assets.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust unit tests.
- `npm run tauri build`: package the desktop app. Prepare the Nginx sidecar first with `./scripts/prepare-nginx.sh` (macOS/Linux) or `scripts/prepare-nginx.ps1` (Windows); see `README.md` for prerequisites.

## Coding Style & Naming Conventions

Follow nearby code: TypeScript uses two-space indentation, single quotes, semicolons, `PascalCase` React components, and `camelCase` functions and variables. Keep strict TypeScript checks passing and put user-facing strings in both locale files. Rust uses standard `rustfmt` formatting, `snake_case` functions/modules, and `PascalCase` types. Check Rust formatting with `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.

## Testing Guidelines

Rust tests are inline `#[cfg(test)]` modules with descriptive `#[test]` names. Add focused tests for backend behavior changes and run the Cargo command above. There is currently no frontend test script or enforced coverage threshold; verify UI changes in `npm run tauri dev`, including relevant states and both languages. Run `npm run build` for frontend changes.

## Commit & Pull Request Guidelines

Recent commits commonly use concise prefixes such as `feat:`, `fix(ui):`, `docs:`, and `refactor(tray):`; use that pattern with a short imperative subject. Keep PRs scoped, explain the behavior changed, link a relevant issue or spec, and list commands actually run. Include screenshots for visible UI changes and note any unverified platform behavior. Never commit credentials, local build caches, or generated Nginx sidecar binaries.
