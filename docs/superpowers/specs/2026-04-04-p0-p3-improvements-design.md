# P0-P3 Improvements Design Spec

## Overview

Comprehensive improvement plan for Meridian covering 18 items across 4 priority tiers: critical fixes (P0), near-term optimizations (P1), mid-term improvements (P2), and future features (P3).

---

## P0: Critical Fixes

### P0-1: Stale Nginx PID Cleanup on Startup

**Problem:** If the app crashes or is `kill -9`'d, the nginx process becomes orphaned. On next launch, the stale PID file causes confusion.

**Design:**
- In `lib.rs` setup, after database init and before auto-start engine logic, add a `cleanup_stale_nginx` function.
- Read `nginx/nginx.pid`. If PID exists but process is not running, delete the PID file.
- If PID exists and process IS running (orphan from previous crash), attempt graceful stop (`nginx -s quit`), then delete PID file if process doesn't exit within 3 seconds, force kill with `kill -9` (Unix) / `taskkill /F` (Windows).
- Log all cleanup actions to error.log via `append_to_error_log`.

**Files changed:**
- `src-tauri/src/nginx_manager/mod.rs` — add `cleanup_stale_process(data_dir)` function
- `src-tauri/src/lib.rs` — call it in setup before auto-start

### P0-2: validate_update_proxy Cross-Field Validation

**Problem:** `validate_update_proxy` only checks `tls_mode` ↔ `certificate_id` when `tls_mode` is present in the update payload. If only `certificate_id` is set to `None` without changing `tls_mode`, a `terminate` rule ends up with no certificate.

**Design:**
- Change `update_proxy` command in `commands/proxy.rs` to load the existing rule from DB before validation.
- Create a new validator function `validate_update_proxy_with_existing(input, existing_rule)` that merges the update fields with existing values to produce a "would-be" state, then validates the merged result with the same rules as `validate_create_proxy`.
- This catches: terminate without cert, passthrough with cert, websocket on stream, domain missing on HTTP, etc.

**Files changed:**
- `src-tauri/src/validators.rs` — add `validate_update_proxy_with_existing(input, existing)`
- `src-tauri/src/commands/proxy.rs` — load existing rule, call new validator

### P0-3: Export Using Native File Dialog

**Problem:** `SettingsPage.tsx` uses `document.createElement('a')` with blob URL to download files. This is unreliable in Tauri WebView (blob URL downloads may not work on all platforms).

**Design:**
- Replace the `<a>` download trick with `@tauri-apps/plugin-dialog`'s `save()` to get a file path from the user.
- Write the JSON data using `@tauri-apps/plugin-fs`'s `writeTextFile()`.
- Similarly, replace the import `<input type="file">` with `@tauri-apps/plugin-dialog`'s `open()` + `@tauri-apps/plugin-fs`'s `readTextFile()`.
- Both `dialog` and `fs` plugins are already configured.

**Files changed:**
- `src/pages/SettingsPage.tsx` — rewrite `handleExport` and `handleImportFile`
- `src/lib/api.ts` — no changes needed (export_data returns the data object)

---

## P1: Near-Term Optimizations

### P1-1: Custom HTTP Headers Editor UI

**Problem:** Backend supports `custom_headers` JSON field on proxy rules, but ProxyForm has no UI to edit it. This is the FR-005 (Should) gap.

**Design:**
- Add a "Custom Headers" section in ProxyForm, visible only for HTTP/HTTPS types (same as TLS section).
- UI: a dynamic key-value list with Add/Remove buttons. Each row has: `Header Name` (Input), `Header Value` (Input), Delete button (X icon).
- Serialize to JSON string `{"X-Custom": "value", ...}` for the `custom_headers` field.
- Parse existing `custom_headers` JSON on edit load.
- Keep it simple: no request/response header distinction (nginx `proxy_set_header` covers request headers; response headers would need `add_header` which is a separate concern — defer to a future enhancement).

**Files changed:**
- `src/components/proxy/ProxyForm.tsx` — add custom headers section
- `src/locales/zh/common.json` — add i18n keys
- `src/locales/en/common.json` — add i18n keys

### P1-2: Accessible Action Buttons (Keyboard/Touch)

**Problem:** Dashboard table action buttons are `opacity-0 group-hover:opacity-100`, invisible to keyboard users and touch devices. Violates NFR-008.

**Design:**
- Change from `opacity-0 group-hover:opacity-100` to `opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-within:opacity-100`.
- Add `tabindex={0}` on each action button (they're already `<button>` elements so they should be focusable — verify they don't have `tabindex={-1}`).
- Add `aria-label` attributes to each icon-only button for screen readers.
- On mobile/touch: use `@media (hover: none)` to show buttons always.

**Files changed:**
- `src/pages/DashboardPage.tsx` — update button container classes and add aria-labels
- `src/index.css` — add `@media (hover: none)` rule for action buttons

### P1-3: Loading Skeleton States

**Problem:** Pages show blank content while data loads, causing visual flash.

**Design:**
- Create a `Skeleton` component (`src/components/ui/Skeleton.tsx`) — a simple animated placeholder with `animate-pulse bg-bg-sidebar rounded`.
- Add skeleton variants: `SkeletonTable` (for Dashboard, Access, Hosts), `SkeletonCards` (for Certs), `SkeletonChart` (for Monitor).
- Each page checks `loading` state from its store and shows skeleton when true.
- Stores already have `loading` field (e.g. `proxy-store.ts:8`) — just need to use it in pages.

**Files changed:**
- `src/components/ui/Skeleton.tsx` — new file, ~60 lines
- `src/pages/DashboardPage.tsx` — add skeleton while loading
- `src/pages/CertsPage.tsx` — add skeleton while loading
- `src/pages/AccessPage.tsx` — add skeleton while loading
- `src/pages/MonitorPage.tsx` — add skeleton while loading
- `src/pages/HostsPage.tsx` — add skeleton while loading
- Stores that don't have `loading` — add loading state (cert-store, access-store, hosts-store)

### P1-4: 404 Catch-All Route

**Problem:** Invalid URLs show a blank page.

**Design:**
- Add a `NotFoundPage` component with a simple message and "Go to Dashboard" button.
- Add `<Route path="*" element={<NotFoundPage />} />` as the last route in App.tsx.

**Files changed:**
- `src/pages/NotFoundPage.tsx` — new file, ~20 lines
- `src/App.tsx` — add catch-all route
- `src/locales/zh/common.json` — add notFound keys
- `src/locales/en/common.json` — add notFound keys

### P1-5: Import Data Auto-Refresh

**Problem:** After importing data, all Zustand stores retain stale state. User must navigate away and back to see new data.

**Design:**
- After successful `importData()` call in SettingsPage, call refresh functions on all relevant stores: `fetchProxies()`, `fetchCertificates()`, `fetchLists()`, `fetchHosts()`.
- Import these store hooks in SettingsPage (some are already imported).

**Files changed:**
- `src/pages/SettingsPage.tsx` — add store refreshes after import

### P1-6: Windows `wmic` Replacement

**Problem:** `nginx_manager/mod.rs:339` uses `wmic` which is deprecated and removed in Windows 11.

**Design:**
- Replace `wmic` command with PowerShell `Get-Process`:
  ```
  powershell -NoProfile -Command "(Get-Process -Id <PID>).StartTime.ToString('yyyyMMddHHmmss')"
  ```
- Parse the output the same way (YYYYMMDDHHmmss format).
- Fallback: if PowerShell fails, return `None` (uptime unknown but not critical).

**Files changed:**
- `src-tauri/src/nginx_manager/mod.rs` — rewrite `get_process_uptime` Windows impl

---

## P2: Mid-Term Improvements

### P2-1: r2d2 Database Connection Pool

**Problem:** Single `Mutex<Connection>` serializes all DB operations. Metrics parsing + log reading block each other.

**Design:**
- Add `r2d2` and `r2d2_sqlite` to Cargo.toml dependencies (r2d2_sqlite wraps rusqlite).
- Replace `AppState.db: Mutex<Connection>` with `AppState.pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>`.
- Pool config: `max_size = 8`, connection init sets `PRAGMA foreign_keys=ON`.
- `AppState.lock_db()` becomes `AppState.get_conn()` returning `r2d2::PooledConnection<...>`.
- WAL mode set once during `init_database` (already done).
- ACME renewal task uses pool.get() instead of opening its own connection, eliminating the bypass.
- `init_database` returns the pool instead of a single connection. Migrations run on one connection from the pool.

**Migration path:**
- All command handlers already use `state.lock_db()?` pattern — search-replace to `state.get_conn()?`.
- Return type changes from `MutexGuard<Connection>` to `PooledConnection<SqliteConnectionManager>` — both deref to `&Connection`, so repo functions need no changes.
- ACME renewal: `r2d2::Pool` is `Clone` (internally Arc-wrapped). Clone the pool and pass it to the spawned task instead of `db_path`. The task calls `pool.get()` instead of `Connection::open(db_path)`.
- Health check task (P2-3): same pattern — receives a pool clone.

**Files changed:**
- `src-tauri/Cargo.toml` — add r2d2, r2d2_sqlite
- `src-tauri/src/store/mod.rs` — return Pool from init_database
- `src-tauri/src/lib.rs` — change AppState, update setup
- `src-tauri/src/acme_client/renewal.rs` — use pool instead of direct Connection::open
- All command files — `lock_db()` → `get_conn()`

### P2-2: Backend Error Code i18n

**Problem:** Backend returns English error strings (e.g., "Name must be between 1 and 100 characters"). Chinese users see English errors.

**Design:**
- Tauri's `InvokeError` only accepts a string. Strategy: serialize the error as a JSON string `{"code": "VALIDATION_NAME_LENGTH", "message": "..."}` in the `From<AppError> for InvokeError` impl.
- Define error code constants in `error.rs` as string codes (not numeric — more descriptive).
- Frontend `parseApiError(e: unknown)` in `src/lib/api.ts`: try `JSON.parse(String(e))` to extract `{code, message}`. If parsing fails (old-style plain string), return `{code: "UNKNOWN", message: String(e)}`.
- i18n: look up `errors.<code>` key, fall back to raw `message` if key not found. This makes the transition gradual — uncoded errors still display.
- Error code categories: `VALIDATION_*`, `CONFLICT_*`, `NOT_FOUND_*`, `NGINX_*`, `CERT_*`, `DNS_*`, `ACME_*`, `DB_*`, `IO_*`.

**Files changed:**
- `src-tauri/src/error.rs` — add structured error serialization with codes
- `src-tauri/src/validators.rs` — use error codes instead of messages
- `src-tauri/src/commands/*.rs` — use error codes for NotFound, Conflict, etc.
- `src/lib/api.ts` — add `parseApiError` helper
- `src/locales/zh/common.json` — add `errors.*` keys
- `src/locales/en/common.json` — add `errors.*` keys
- All frontend pages — use `parseApiError` instead of `String(e)`

### P2-3: Nginx Health Check + Notification

**Problem:** If nginx crashes during runtime, the user has no indication until they manually check.

**Design:**
- Add a background health check loop in Rust, similar to ACME renewal:
  - Run every 15 seconds
  - Check `nginx_manager::status()` — if status was "running" last check but is now "stopped"/"error", emit a Tauri event `nginx-status-changed`.
  - Store last known status in a static `AtomicBool` or small struct.
- Frontend: listen for `nginx-status-changed` event in `App.tsx`.
  - Show a persistent warning toast: "Engine stopped unexpectedly" with a "Restart" action button.
  - Update engine store status.
  - Also add a badge/indicator in the sidebar next to engine status.
- The health check also keeps the engine store's status accurate even when the user isn't on the dashboard.

**Files changed:**
- `src-tauri/src/lib.rs` — spawn health check task in setup
- `src-tauri/src/nginx_manager/mod.rs` — add `spawn_health_check(data_dir, app_handle)` function
- `src/App.tsx` — listen for `nginx-status-changed` event
- `src/stores/engine-store.ts` — handle status change event
- `src/components/ui/Toast.tsx` — support action button in toast (optional, or use a dedicated banner)
- `src/locales/*/common.json` — add notification keys

### P2-4: Scheduled Log Cleanup

**Problem:** Log cleanup only runs at startup. Long-running instances accumulate logs indefinitely.

**Design:**
- Spawn a background task (like ACME renewal) that runs log cleanup:
  - Initial delay: 60 seconds after startup (after the first cleanup already ran).
  - Interval: every 6 hours.
  - Reads `log_retention_days` setting from DB each time.
  - Calls existing `cleanup_old_logs()`.
- Reuse the pattern from `acme_client/renewal.rs::spawn_renewal_task`.

**Files changed:**
- `src-tauri/src/commands/logs.rs` — extract cleanup into a public helper, add `spawn_log_cleanup_task`
- `src-tauri/src/lib.rs` — spawn the task in setup

### P2-5: Unit Tests for Core Modules

**Problem:** Only `hosts_manager.rs` has unit tests. Critical modules like config engine, validators, and metrics parser have none.

**Design:**

**validators.rs tests (~20 tests):**
- Valid/invalid create proxy (each field boundary)
- Valid/invalid update proxy with existing rule merge
- IP/CIDR validation (IPv4, IPv6, CIDR ranges, edge cases)
- Hostname validation (valid, too long, invalid chars)
- Certificate domain validation

**config_engine tests (~15 tests):**
- `main_config.rs` — generates valid nginx.conf structure
- `http_config.rs` — correct server block for single rule, grouped rules (same port+domain), TLS terminate, WebSocket, access list, custom headers
- `stream_config.rs` — TCP block, UDP block, stream TLS
- `conflict.rs` — all 5 conflict scenarios (same port+domain+path, same port diff domain OK, stream same port, http vs stream)

**metrics/parser.rs tests (~8 tests):**
- Parse valid HTTP JSON log line
- Parse valid stream JSON log line
- Handle malformed lines gracefully
- Aggregator: correct bucket sizing for 1h/6h/24h ranges

**Test approach:**
- All tests use `#[cfg(test)] mod tests` within each file.
- Config engine tests assert on generated string content (contains expected directives).
- No external dependencies needed (no filesystem, no nginx binary).

**Files changed:**
- `src-tauri/src/validators.rs` — add `#[cfg(test)] mod tests`
- `src-tauri/src/config_engine/main_config.rs` — add tests
- `src-tauri/src/config_engine/http_config.rs` — add tests
- `src-tauri/src/config_engine/stream_config.rs` — add tests
- `src-tauri/src/config_engine/conflict.rs` — add tests
- `src-tauri/src/metrics/parser.rs` — add tests
- `src-tauri/src/metrics/aggregator.rs` — add tests

---

## P3: Future Features

### P3-1: Batch Operations

**Problem:** No way to enable/disable/delete multiple proxy rules at once.

**Design:**
- Add checkbox column to Dashboard table (leftmost).
- "Select All" checkbox in header.
- When any rows selected, show a floating action bar at the bottom: "N selected — Enable | Disable | Delete".
- Backend: add `batch_toggle_proxies(ids, enabled)` and `batch_delete_proxies(ids)` commands.
- Single `apply_and_reload` after batch operation completes.

**State management:**
- Selection state lives in DashboardPage local state (`selectedIds: Set<string>`), not in the store.
- Clear selection after batch action completes.

**Files changed:**
- `src-tauri/src/commands/proxy.rs` — add batch commands
- `src-tauri/src/store/proxy_repo.rs` — add batch DB operations
- `src-tauri/src/lib.rs` — register new commands
- `src/pages/DashboardPage.tsx` — add selection UI and floating bar
- `src/lib/api.ts` — add batch API functions
- `src/locales/*/common.json` — add batch action keys

### P3-2: First-Run Onboarding

**Problem:** New users see an empty dashboard with no guidance.

**Design:**
- Detect first run via `app_settings` table: check for key `onboarding_completed`.
- If not completed, show an onboarding overlay on DashboardPage:
  - Step 1: Welcome message + brief explanation of what Meridian does.
  - Step 2: "Create your first proxy rule" CTA button → navigates to `/proxy/new`.
  - "Skip" link to dismiss.
- After first proxy is created OR user clicks Skip, set `onboarding_completed = true`.
- Keep it minimal — not a multi-step wizard, just a single welcome card in the empty state area.
- Reuse the existing empty state in DashboardPage, enhance it with more descriptive text and prominent CTA.

**Files changed:**
- `src/pages/DashboardPage.tsx` — enhanced empty state with onboarding
- `src/lib/api.ts` — getSetting/setSetting already exist
- `src/locales/*/common.json` — add onboarding keys

### P3-3: Multi-Upstream Load Balancing (FR-007)

**Problem:** Each proxy rule supports only one upstream target. FR-007 (Could priority) requests simple round-robin load balancing.

**Design:**
- Add `upstream_targets` field to `proxy_rules` table (JSON array: `[{"host":"127.0.0.1","port":3000,"weight":1},...]`). Keep existing `upstream_host`/`upstream_port` as the primary (backwards compatible).
- If `upstream_targets` is null/empty, use `upstream_host:upstream_port` (current behavior).
- If `upstream_targets` has entries, generate an `upstream` block in nginx config with all targets.
- Supported methods: round-robin (default), weighted (via weight field). No need for ip_hash or least_conn for local dev use.
- ProxyForm UI: "Add upstream target" button below the existing upstream fields. Dynamic list of host:port:weight rows.

**Database migration:**
- `ALTER TABLE proxy_rules ADD COLUMN upstream_targets TEXT;` — JSON field, nullable.

**Config engine changes:**
- `http_config.rs`: when rule has upstream_targets, generate `upstream meridian_<rule_id> { server host:port weight=N; ... }` block and use `proxy_pass http://meridian_<rule_id>` instead of direct host:port.
- `stream_config.rs`: same pattern with `proxy_pass meridian_<rule_id>`.

**Files changed:**
- `src-tauri/src/store/mod.rs` — add migration
- `src-tauri/src/store/models.rs` — add `upstream_targets` field
- `src-tauri/src/store/proxy_repo.rs` — handle new field in CRUD
- `src-tauri/src/config_engine/http_config.rs` — generate upstream blocks
- `src-tauri/src/config_engine/stream_config.rs` — generate upstream blocks
- `src-tauri/src/validators.rs` — validate upstream_targets JSON
- `src/types/index.ts` — add UpstreamTarget type
- `src/components/proxy/ProxyForm.tsx` — add multi-upstream UI
- `src/locales/*/common.json` — add upstream keys

---

## Implementation Order

The items should be implemented in dependency order:

**Phase 1 (P0 — no dependencies):**
1. P0-1: Stale PID cleanup
2. P0-2: Validator cross-field fix
3. P0-3: Export native dialog

**Phase 2 (P1 — no dependencies on P0):**
4. P1-4: 404 route (trivial, do first)
5. P1-5: Import auto-refresh (trivial)
6. P1-6: Windows wmic replacement
7. P1-2: Accessible action buttons
8. P1-3: Loading skeletons (needs store loading states)
9. P1-1: Custom headers UI

**Phase 3 (P2 — r2d2 pool first, others parallel):**
10. P2-1: r2d2 connection pool (do first — other P2 items benefit from it)
11. P2-4: Scheduled log cleanup (can parallel with P2-2)
12. P2-3: Nginx health check (can parallel with P2-2)
13. P2-2: Error code i18n (large surface area, do last in P2)
14. P2-5: Unit tests (run throughout, but main push after code changes stabilize)

**Phase 4 (P3 — sequential):**
15. P3-2: Onboarding (trivial, do first)
16. P3-1: Batch operations
17. P3-3: Multi-upstream load balancing (largest scope, do last)

## Risk Assessment

| Item | Risk | Mitigation |
|------|------|------------|
| P2-1 r2d2 pool | High: touches every command handler | Mechanical search-replace; pool API is drop-in compatible |
| P2-2 Error i18n | Medium: large surface area | Phase gradually; fallback to raw message ensures no regression |
| P3-3 Multi-upstream | Medium: config engine changes | Existing rules unaffected (null upstream_targets = old behavior) |
| P2-3 Health check | Low: additive feature | 15s polling is lightweight; no impact on existing code |
