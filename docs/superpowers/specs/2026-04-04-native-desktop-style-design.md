# Native Desktop Style — Design Spec

## Goal

Transform Meridian from a web-like appearance into a platform-native desktop application. Each platform (macOS, Windows 11, Linux/GTK) receives its own visual treatment using native window effects and platform-appropriate styling.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Platform strategy | Per-platform native adaptation | macOS vibrancy, Windows Mica, Linux GTK — not a single cross-platform style |
| macOS sidebar material | 55% opacity white glass (`rgba(255,255,255,0.55)`) | Finder-like translucency; neither too opaque nor too transparent |
| Dark mode | Follow system automatically | Native materials (vibrancy, Mica) auto-adapt; no custom dark overrides needed |
| Controls layout | Engine status in content toolbar (Option B) | Sidebar stays pure navigation; content toolbar holds page title + engine status + action buttons; best cross-platform consistency |
| Theme/language toggles | Move to Settings page | Low-frequency operations; removes noise from persistent UI |

## 1. Layout Restructure

### Current (Before)

- `decorations: true` — system native titlebar always visible
- Custom `Titlebar` component (engine status, start/stop/reload, theme toggle, language toggle)
- Two-layer titlebar: ~80px vertical waste
- Sidebar: flat solid color `#f5f5f4`, identical across all platforms

### After

- **macOS**: Hide native titlebar; traffic lights embedded in sidebar top via `titleBarStyle: "overlay"`
- **Windows / Linux**: Keep native titlebar; remove custom `Titlebar` component entirely
- Sidebar becomes pure navigation
- New `ContentToolbar` component in the content area replaces the old Titlebar's functionality
- Single-layer header on all platforms

### Grid Layout Changes

**macOS** (no system titlebar):
```
grid-template-columns: 220px 1fr
grid-template-rows: 1fr
```
Sidebar spans full height. Content area has its own toolbar row internally.

**Windows / Linux** (system titlebar present):
```
grid-template-columns: 220px 1fr
grid-template-rows: 1fr
```
Same grid — the system titlebar is outside the webview, so no `grid-rows` change needed. Remove the current 48px titlebar row from the grid.

## 2. Platform Visual Specifications

### macOS

| Property | Value |
|----------|-------|
| Titlebar | Hidden via `titleBarStyle: "overlay"` in `tauri.macos.conf.json` (keeps traffic lights visible) |
| Traffic lights | System-rendered via overlay; sidebar top reserves 52px for traffic lights + app name |
| Sidebar material | `NSVisualEffectView` (`.sidebar` material) applied to full window via `objc2-app-kit` (already a dependency) |
| Sidebar CSS | `background: transparent` — native layer provides the blur; content area uses opaque `background-color` to mask the vibrancy |
| Drag region | Sidebar top area + content toolbar (via `data-tauri-drag-region`) |
| Accent color | `#2563eb` |
| Border radius | 6–8px |
| Font | System default (SF Pro) |

### Windows 11

| Property | Value |
|----------|-------|
| Titlebar | Native system titlebar (minimize/maximize/close) |
| Sidebar material | Mica applied to full window via `window-vibrancy` crate; fallback to solid `#f3f3f3` on pre-Win11 |
| Sidebar CSS | `background: transparent` — Mica shows through; content area uses opaque `background-color` to mask |
| Accent color | `#0067c0` |
| Border radius | 4px (Windows 11 convention) |
| Font | System default (Segoe UI) |

### Linux / GTK

| Property | Value |
|----------|-------|
| Titlebar | Native system titlebar (DE-dependent) |
| Sidebar material | Solid color, no transparency (compatibility-first) |
| Sidebar CSS | Subtle flat background — Adwaita-inspired `#f6f5f4` light / `#2d2d2d` dark |
| Accent color | `#3584e4` (GNOME blue) |
| Border radius | 6–8px (Adwaita style) |
| Font | System default (Cantarell / Noto Sans) |

### Dark Mode (All Platforms)

Follow system preference. Native materials auto-adapt:
- macOS: NSVisualEffectView switches to dark variant automatically
- Windows: Mica dark variant
- Linux: CSS dark theme tokens applied via `.dark` class (existing mechanism)

No custom dark-mode sidebar overrides needed.

## 3. Component Changes

### Delete: `Titlebar` component

The entire `src/components/layout/Titlebar.tsx` is removed. Its responsibilities are redistributed:
- Engine status pill + start/stop/reload → `ContentToolbar`
- Theme toggle → Settings page
- Language toggle → Settings page
- App name display → Sidebar top (macOS) / system titlebar (Windows, Linux)

### New: `ContentToolbar` component

Replaces the old Titlebar inside the content area. Layout:
```
[Page Title]                    [Engine Status Pill ⟳ ■ ▶] [Page Actions...]
```

- Left: page title (e.g., "仪表盘")
- Right: engine status indicator with inline control buttons, then page-specific actions (e.g., "+ 添加代理")
- On macOS, the entire toolbar is a drag region (`data-tauri-drag-region`)
- Height: 48px, consistent across platforms

### Modified: `Sidebar` component

- Remove all non-navigation elements (engine status was already elsewhere; theme/language toggles removed)
- macOS: add 52px top padding for traffic light area + app name
- Windows/Linux: standard top padding, no traffic light space needed
- Platform detection via `data-platform` attribute on root element

### Modified: `AppShell` component

- Remove titlebar row from grid
- New grid: `grid-cols-[220px_1fr] h-screen` (two columns, single row)
- Set `data-platform` attribute on the root `<div>` based on platform detection

### Modified: Settings page

- Add theme selector (light / dark / system) — migrated from Titlebar
- Add language selector — migrated from Titlebar

### Enhanced: Cards and Tables

- Add subtle box-shadow: `0 1px 3px rgba(0,0,0,0.06)` on cards
- Table row hover: more visible background change with `transition: background 150ms ease`
- Buttons: subtle press feedback via `active:scale-[0.98]` transform

## 4. CSS Architecture

### Platform Detection

Rust backend provides a `get_platform` IPC command returning `"macos" | "windows" | "linux"`.

Frontend `usePlatform()` hook calls this once on mount and sets `data-platform` attribute on `<html>`:

```html
<html data-platform="macos" class="dark">
```

### Platform-Specific CSS Tokens

In `index.css`, platform overrides applied via attribute selectors:

```css
/* macOS */
[data-platform="macos"] {
  --color-accent: #2563eb;
  --color-sidebar-bg: transparent;
  --radius-sm: 6px;
  --radius-md: 8px;
}

/* Windows */
[data-platform="windows"] {
  --color-accent: #0067c0;
  --color-sidebar-bg: transparent;
  --radius-sm: 4px;
  --radius-md: 4px;
}

/* Linux */
[data-platform="linux"] {
  --color-accent: #3584e4;
  --color-sidebar-bg: #f6f5f4;
  --radius-sm: 6px;
  --radius-md: 8px;
}

/* Linux dark */
.dark[data-platform="linux"] {
  --color-sidebar-bg: #2d2d2d;
}
```

### Sidebar Top Spacing

```css
[data-platform="macos"] .sidebar-nav {
  padding-top: 52px; /* traffic light + app name space */
}
```

## 5. Rust Backend Changes

### Tauri Window Configuration

**macOS-specific** (via `tauri.macos.conf.json`):
- `titleBarStyle: "overlay"` — hides titlebar but keeps traffic light buttons
- `transparent: true` — allows NSVisualEffectView to show through the sidebar

**Windows / Linux** (`tauri.conf.json` default):
- `decorations: true` (keep native titlebar)

### NSVisualEffectView (macOS)

Use existing `objc2-app-kit` dependency to apply vibrancy to the window's sidebar region. This is done in the Rust setup hook after window creation:
- Get the content view
- Create an NSVisualEffectView with `.sidebar` material
- Add it as a subview or set it as the window's background

### Mica (Windows)

Add `window-vibrancy` crate to apply Mica material to the full window background. The sidebar CSS is `background: transparent` so Mica shows through; the content area CSS uses an opaque background to mask it. Fallback: if the OS is pre-Windows 11 (no Mica support), sidebar uses solid `#f3f3f3` (light) / `#2d2d2d` (dark).

### `get_platform` IPC Command

```rust
#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") { "macos" }
    else if cfg!(target_os = "windows") { "windows" }
    else { "linux" }
}
```

## 6. Out of Scope

The following are explicitly excluded from this work:

- Custom window control buttons (Windows/Linux keep native)
- Dynamic system accent color reading (fixed platform colors)
- Resizable sidebar width
- Animation/motion system beyond basic hover/transition
- Per-page custom toolbar layouts (all pages share the same ContentToolbar)
