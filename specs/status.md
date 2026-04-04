# Project Status: 轻渡 · Meridian

## Trunk Health

| Metric | Value |
|--------|-------|
| Active features | 8 (store, proxy, config_engine, nginx_mgr, cert, access, ui, monitoring) |
| Last trunk update | 2026-04-04 (FEAT-001 + FEAT-002 merged to trunk) |
| Open deferred items | 2 |

## Initial Build Progress

| Phase | Status | Artifact | Gate |
|-------|--------|----------|------|
| 0. Scan | N/A | greenfield project | — |
| 1. Clarify | ✅ Done | requirements.md | Gate 1 ✅ |
| 2a–2b. Design | ✅ Done | design.md + preview | Gate 2 ✅ |
| 2c–2d. Plan | ✅ Done | tasks.md + 7 specs | Gate 3 ✅ |
| 3. Generate | ✅ Done | code + tests | Gate 4 ✅ |
| 4. Verify | ✅ Done | build verified | — |

## Build Results

- **Rust backend**: 30 Tauri IPC commands, clean compilation (0 warnings)
- **React frontend**: 6 pages, 7 UI components, 6 Zustand stores, complete i18n (zh/en)
- **Vite build**: ✅ success (375KB JS + 26KB CSS)
- **Tauri build**: ✅ .app bundle + binary (16MB)
- **Gate 4 review**: 3 Critical + 14 Major issues found → ALL FIXED

## Gate 4 Fixes Applied
1. ✅ Input validation layer (`validators.rs`)
2. ✅ Reference checks on cert/access list delete
3. ✅ Transactional config flow with backup/rollback
4. ✅ `check_port_conflict` command with exclude_id
5. ✅ Access list improvements (detail response, reorder, dedup, cascade reload)
6. ✅ `count_by_type` + filtered `list_proxies` with stats
7. ✅ Update proxy allows clearing optional fields
8. ✅ Engine status/restart per spec shape

## Post-Gate 4 Fixes & Features
1. ✅ Nginx config path quoting — all paths in generated nginx configs wrapped in double quotes (fixes `Application Support` space issue)
2. ✅ Nginx stop/reload `-c` flag — pass custom config path so nginx finds correct pid file
3. ✅ Nginx lifecycle logging — start/stop/reload/test events written to error.log with `[meridian]` tag
4. ✅ Engine status label — removed "Nginx" prefix ("运行中"/"Running")
5. ✅ System tray integration — full right-click menu: status display, start/stop, add proxy, show window, quit
6. ✅ Close-to-tray — window close hides to tray, app runs in background
7. ✅ Tray menu i18n — menu text follows app language setting (zh/en)
8. ✅ Tray menu state sync — items enabled/disabled based on engine status, refreshed after each action
9. ✅ Custom app icon — blue gradient + flowing stream lines (SVG source, all sizes generated)

## Merged Features
1. ✅ FEAT-001: ACME DNS-01 Certificate Management — 4 DNS providers, wildcard/SAN certs, auto-renewal
2. ✅ FEAT-002: Proxy Monitoring Dashboard — recharts-based, per-rule filtering, 3 chart types, nginx JSON log parsing
3. ✅ Custom Select component — replaced native select with styled dropdown
4. ✅ LogsPage enhancements — auto-refresh (2s), smart scroll, per-rule filter
5. ✅ ProxyForm validation — field error highlighting, default ports (HTTP→80, HTTPS→443)
6. ✅ Tray menu refinements — removed reload, renamed to "添加代理", left-click shows window only
7. ✅ Log retention — configurable (1-365 days, default 7), startup cleanup
8. ✅ Sidebar cleanup — removed duplicate "Add Proxy" entry

## Remaining Minor Items (deferred, non-blocking)
- Real-time log tailing (Tauri event stream)
- Nginx path / data dir settings in Settings page

## Codebase Context

- **Tech Stack:** Tauri v2 + React 18 + TypeScript + Tailwind CSS 4 + Rust + SQLite
- **Source files:** ~25 Rust modules + ~30 TypeScript files
- **App Icon:** Custom SVG → PNG/ICO/ICNS (blue gradient + stream lines)
- **System Tray:** Full menu with engine control, i18n, state sync
- **Charting:** recharts (AreaChart, LineChart, PieChart) for monitoring
- **Monitoring:** On-demand nginx JSON log parsing, no persistent metrics DB

## Active Changes

(None)

## Last Updated: 2026-04-04
