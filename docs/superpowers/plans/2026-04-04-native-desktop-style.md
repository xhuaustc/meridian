# Native Desktop Style Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Meridian from a web-like appearance into a platform-native desktop app with macOS vibrancy, Windows Mica, and Linux GTK styling.

**Architecture:** Platform detection via a Rust IPC command sets a `data-platform` attribute on `<html>`. CSS tokens vary per platform. The old `Titlebar` component is removed; engine controls move to a new `ContentToolbar` in the content area. macOS hides the native titlebar and uses NSVisualEffectView for sidebar vibrancy. Windows applies Mica via `window-vibrancy` crate. Linux uses solid GTK-inspired colors.

**Tech Stack:** Tauri v2, React, Tailwind CSS v4, `objc2-app-kit` (macOS vibrancy), `window-vibrancy` (Windows Mica)

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src-tauri/tauri.macos.conf.json` | macOS-only window config (overlay titlebar, transparent) |
| Create | `src/hooks/usePlatform.ts` | Platform detection hook via IPC |
| Create | `src/components/layout/ContentToolbar.tsx` | Engine status + page title + actions toolbar |
| Modify | `src-tauri/Cargo.toml` | Add `window-vibrancy` dependency |
| Modify | `src-tauri/src/lib.rs` | Add `get_platform` command, apply vibrancy/Mica in setup |
| Modify | `src/lib/api.ts` | Add `getPlatform()` IPC call |
| Modify | `src/App.tsx` | Set `data-platform` on `<html>`, remove Titlebar dep |
| Modify | `src/index.css` | Add platform-specific CSS token overrides |
| Modify | `src/components/layout/AppShell.tsx` | Remove titlebar row, use new ContentToolbar |
| Modify | `src/components/layout/Sidebar.tsx` | Add macOS traffic-light padding, transparent bg |
| Modify | `src/pages/DashboardPage.tsx` | Move page-specific actions into ContentToolbar pattern |
| Delete | `src/components/layout/Titlebar.tsx` | Replaced by ContentToolbar |

---

### Task 1: Add `get_platform` Rust Command

**Files:**
- Modify: `src-tauri/src/lib.rs:386-444`

- [ ] **Step 1: Add the `get_platform` command**

Add this function before the `pub fn run()` function in `src-tauri/src/lib.rs`:

```rust
#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}
```

- [ ] **Step 2: Register the command in the invoke handler**

In the `.invoke_handler(tauri::generate_handler![...])` block, add `get_platform` after `sync_tray`:

```rust
            sync_tray,
            get_platform,
        ])
```

- [ ] **Step 3: Add the frontend API call**

In `src/lib/api.ts`, add at the bottom:

```typescript
export const getPlatform = () => invoke<string>('get_platform');
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src/lib/api.ts
git commit -m "feat: add get_platform IPC command for platform detection"
```

---

### Task 2: Create `usePlatform` Hook and Set `data-platform`

**Files:**
- Create: `src/hooks/usePlatform.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create the platform hook**

Create `src/hooks/usePlatform.ts`:

```typescript
import { useEffect, useState } from 'react';
import { getPlatform } from '../lib/api';

export type Platform = 'macos' | 'windows' | 'linux';

let cached: Platform | null = null;

export function usePlatform(): Platform | null {
  const [platform, setPlatform] = useState<Platform | null>(cached);

  useEffect(() => {
    if (cached) return;
    getPlatform().then((p) => {
      cached = p as Platform;
      setPlatform(cached);
    });
  }, []);

  return platform;
}
```

- [ ] **Step 2: Set `data-platform` on `<html>` in App.tsx**

In `src/App.tsx`, add the import and effect. After the existing imports, add:

```typescript
import { usePlatform } from './hooks/usePlatform';
```

Inside the `App` function, after the existing `useEffect` blocks, add:

```typescript
  const platform = usePlatform();
  useEffect(() => {
    if (platform) {
      document.documentElement.setAttribute('data-platform', platform);
    }
  }, [platform]);
```

- [ ] **Step 3: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add src/hooks/usePlatform.ts src/App.tsx
git commit -m "feat: add usePlatform hook and set data-platform attribute on html"
```

---

### Task 3: Add Platform-Specific CSS Tokens

**Files:**
- Modify: `src/index.css`

- [ ] **Step 1: Add platform token overrides after the dark mode block**

In `src/index.css`, after the `.dark { ... }` block (after line 56), add:

```css
/* Platform-specific overrides */
[data-platform="macos"] {
  --color-accent: #2563eb;
  --color-bg-sidebar: transparent;
  --radius-sm: 6px;
  --radius-md: 8px;
}

[data-platform="windows"] {
  --color-accent: #0067c0;
  --color-accent-light: #e8f0fe;
  --color-bg-sidebar: transparent;
  --radius-sm: 4px;
  --radius-md: 4px;
}

[data-platform="linux"] {
  --color-accent: #3584e4;
  --color-accent-light: #dce8fc;
  --color-bg-sidebar: #f6f5f4;
  --radius-sm: 6px;
  --radius-md: 8px;
}

/* Dark mode + platform overrides */
.dark[data-platform="macos"] {
  --color-accent: #3b82f6;
  --color-bg-sidebar: transparent;
}

.dark[data-platform="windows"] {
  --color-accent: #4cc2ff;
  --color-accent-light: #1a3a5c;
  --color-bg-sidebar: transparent;
}

.dark[data-platform="linux"] {
  --color-accent: #62a0ea;
  --color-accent-light: #1a3352;
  --color-bg-sidebar: #2d2d2d;
}

/* macOS sidebar top padding for traffic lights */
[data-platform="macos"] .sidebar-nav {
  padding-top: 52px;
}
```

- [ ] **Step 2: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/index.css
git commit -m "feat: add platform-specific CSS token overrides"
```

---

### Task 4: Create `ContentToolbar` Component

**Files:**
- Create: `src/components/layout/ContentToolbar.tsx`

- [ ] **Step 1: Create the ContentToolbar component**

This component replaces the old Titlebar. It shows page title on the left, engine status + controls on the right.

Create `src/components/layout/ContentToolbar.tsx`:

```typescript
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, RotateCw } from 'lucide-react';
import { useEngineStore } from '../../stores/engine-store';
import { useSettingsStore } from '../../stores/settings-store';
import { cn } from '../../lib/utils';

interface ContentToolbarProps {
  title: string;
  children?: React.ReactNode;
}

export function ContentToolbar({ title, children }: ContentToolbarProps) {
  const { t } = useTranslation('common');
  const { status, loading, fetchStatus, start, stop, reload } = useEngineStore();
  const theme = useSettingsStore((s) => s.theme);

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Listen for system color scheme changes (migrated from old Titlebar)
  useEffect(() => {
    if (theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = () => useSettingsStore.getState().applyTheme('system');
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }
  }, [theme]);

  const isRunning = status?.status === 'running';

  return (
    <div
      className="h-12 flex items-center justify-between px-5 border-b border-border shrink-0"
      data-tauri-drag-region
    >
      <h1
        className="text-[15px] font-semibold tracking-[-0.01em] text-text-primary"
        data-tauri-drag-region
      >
        {title}
      </h1>
      <div className="flex items-center gap-3">
        {/* Engine status pill */}
        <div
          className={cn(
            'flex items-center gap-1.5 px-2.5 py-1 rounded-[20px] text-[11px] font-medium',
            isRunning
              ? 'bg-success-bg text-success'
              : 'bg-error-bg text-error',
          )}
        >
          <span
            className={cn(
              'w-1.5 h-1.5 rounded-full bg-current',
              isRunning && 'animate-pulse',
            )}
          />
          {isRunning ? t('engine.running') : t('engine.stopped')}
        </div>

        {/* Engine controls */}
        <div className="flex items-center gap-1">
          {!isRunning ? (
            <button
              onClick={start}
              disabled={loading}
              className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-success disabled:opacity-50"
              title={t('engine.start')}
            >
              <Play className="w-3.5 h-3.5" />
            </button>
          ) : (
            <>
              <button
                onClick={reload}
                disabled={loading}
                className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-accent disabled:opacity-50"
                title={t('engine.reload')}
              >
                <RotateCw className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={stop}
                disabled={loading}
                className="p-1.5 rounded hover:bg-bg-hover text-text-secondary hover:text-error disabled:opacity-50"
                title={t('engine.stop')}
              >
                <Square className="w-3.5 h-3.5" />
              </button>
            </>
          )}
        </div>

        {/* Page-specific actions */}
        {children}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/layout/ContentToolbar.tsx
git commit -m "feat: create ContentToolbar component with engine status and controls"
```

---

### Task 5: Refactor AppShell — Remove Titlebar, Integrate ContentToolbar

**Files:**
- Modify: `src/components/layout/AppShell.tsx`
- Modify: `src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Rewrite AppShell**

Replace the entire content of `src/components/layout/AppShell.tsx` with:

```typescript
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';

export function AppShell() {
  return (
    <div className="grid grid-cols-[220px_1fr] h-screen">
      <Sidebar />
      <main className="overflow-y-auto flex flex-col">
        <Outlet />
      </main>
    </div>
  );
}
```

Key changes: removed Titlebar import/render, removed `grid-rows-[48px_1fr]` (no titlebar row), main is now `flex flex-col` so ContentToolbar + scrollable content work together.

- [ ] **Step 2: Add `sidebar-nav` class and macOS padding to Sidebar**

In `src/components/layout/Sidebar.tsx`, change the outer `<div>` class. Replace:

```typescript
    <div className="bg-bg-sidebar border-r border-border py-3 px-2 flex flex-col gap-0.5 overflow-y-auto">
```

With:

```typescript
    <div className="sidebar-nav bg-bg-sidebar border-r border-border py-3 px-2 flex flex-col gap-0.5 overflow-y-auto">
```

This adds the `sidebar-nav` class that the CSS in Task 3 targets for macOS traffic-light padding.

- [ ] **Step 3: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/AppShell.tsx src/components/layout/Sidebar.tsx
git commit -m "refactor: remove Titlebar from AppShell, add sidebar-nav class"
```

---

### Task 6: Migrate Each Page to Use ContentToolbar

Every page currently renders its own `<h1>` title and action buttons inline. They now need to wrap their content with `ContentToolbar`. The toolbar sits at the top of the page (inside `<main>`) and the scrollable content goes below it.

**Files:**
- Modify: `src/pages/DashboardPage.tsx`
- Modify: `src/pages/MonitorPage.tsx`
- Modify: `src/pages/LogsPage.tsx`
- Modify: `src/pages/CertsPage.tsx`
- Modify: `src/pages/AccessPage.tsx`
- Modify: `src/pages/SettingsPage.tsx`
- Modify: `src/pages/ProxyFormPage.tsx`

- [ ] **Step 1: Migrate DashboardPage**

In `src/pages/DashboardPage.tsx`, add the import at the top:

```typescript
import { ContentToolbar } from '../components/layout/ContentToolbar';
```

Find the existing header block (the `<div className="flex items-center justify-between mb-5">` containing the `<h1>` and the search/filter/add-proxy buttons). Wrap the entire page return in a fragment with `ContentToolbar` at the top. The page title and the "Add Proxy" button move into ContentToolbar. The search bar and type filter remain in the page body.

Replace the page's return starting from `<div>` through the header `</div>` with this structure:

```tsx
    <>
      <ContentToolbar title={t('dashboard.title')}>
        <Button size="sm" onClick={() => navigate('/proxy/new')}>
          <Plus className="w-3.5 h-3.5" />
          {t('dashboard.addProxy')}
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
        {/* search bar, filter pills, table, etc. — everything that was below the old header */}
```

Close with `</div></>` at the bottom instead of the old single `</div>`.

Remove the old `<h1>` tag and the outer `<div className="flex items-center justify-between mb-5">` wrapper since the title and add button are now in ContentToolbar.

- [ ] **Step 2: Migrate MonitorPage**

Same pattern. Add `ContentToolbar` import. Wrap return in `<>`. Move the `<h1>` title text into `<ContentToolbar title={t('monitor.title')}>`. The time range selector and refresh button become `children` of ContentToolbar. The charts and stats go inside a `<div className="p-6 overflow-y-auto flex-1">`.

- [ ] **Step 3: Migrate LogsPage**

Add `ContentToolbar` import. The title goes into ContentToolbar. The access/error tab buttons, proxy selector, auto-refresh toggle, and clear button become `children` of ContentToolbar. The log viewer goes inside `<div className="p-6 overflow-y-auto flex-1">`.

- [ ] **Step 4: Migrate CertsPage**

Add `ContentToolbar` import. Title in toolbar. Action buttons (add cert) as children. Content in `<div className="p-6 overflow-y-auto flex-1">`.

- [ ] **Step 5: Migrate AccessPage**

Same pattern as CertsPage.

- [ ] **Step 6: Migrate SettingsPage**

Add `ContentToolbar` import. Title in toolbar, no action children needed. Content in `<div className="p-6 overflow-y-auto flex-1">`.

- [ ] **Step 7: Migrate ProxyFormPage**

Add `ContentToolbar` import. Title (dynamic: "Add Proxy" or "Edit Proxy") in toolbar. Save/cancel buttons as children. Form content in `<div className="p-6 overflow-y-auto flex-1">`.

- [ ] **Step 8: Delete the old Titlebar component**

Delete `src/components/layout/Titlebar.tsx`. It is no longer imported anywhere.

- [ ] **Step 9: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "refactor: migrate all pages to ContentToolbar, delete old Titlebar"
```

---

### Task 7: macOS — Overlay Titlebar Configuration

**Files:**
- Create: `src-tauri/tauri.macos.conf.json`

- [ ] **Step 1: Create macOS-specific Tauri config**

Create `src-tauri/tauri.macos.conf.json`:

```json
{
  "app": {
    "windows": [
      {
        "titleBarStyle": "overlay",
        "hiddenTitle": true,
        "transparent": true
      }
    ]
  }
}
```

Tauri v2 automatically merges platform-specific config files. This hides the titlebar but keeps the traffic light buttons, and enables transparency for the vibrancy effect.

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.macos.conf.json
git commit -m "feat: add macOS overlay titlebar config"
```

---

### Task 8: macOS — Apply NSVisualEffectView for Sidebar Vibrancy

**Files:**
- Modify: `src-tauri/Cargo.toml:44-46`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add required objc2-app-kit features**

In `src-tauri/Cargo.toml`, update the `objc2-app-kit` dependency to include the required features for NSVisualEffectView. Replace:

```toml
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSRunningApplication"] }
```

With:

```toml
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSRunningApplication", "NSView", "NSVisualEffectView", "NSWindow", "NSResponder"] }
```

- [ ] **Step 2: Add vibrancy setup function in lib.rs**

In `src-tauri/src/lib.rs`, add this function after the existing `set_dock_visible` function (after line 38):

```rust
/// Apply NSVisualEffectView to the entire window for sidebar vibrancy.
/// The sidebar CSS uses `background: transparent` so the vibrancy shows through,
/// while the content area uses an opaque background to mask it.
#[cfg(target_os = "macos")]
fn apply_vibrancy(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSVisualEffectMaterial, NSVisualEffectView, NSVisualEffectBlendingMode};
    use objc2_foundation::{NSObjectProtocol, MainThreadMarker};
    use tauri::Manager;

    let ns_window = window.ns_window().unwrap() as *mut objc2_app_kit::NSWindow;
    let ns_window = unsafe { &*ns_window };

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let effect_view = unsafe { NSVisualEffectView::new(mtm) };
    effect_view.setMaterial(NSVisualEffectMaterial::Sidebar);
    effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect_view.setState(objc2_app_kit::NSVisualEffectState::FollowsWindowActiveState);

    let content_view = unsafe { ns_window.contentView().unwrap() };
    effect_view.setFrame(content_view.frame());
    effect_view.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::NSViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::NSViewHeightSizable,
    );

    unsafe {
        content_view.addSubview_positioned_relativeTo(
            &effect_view,
            objc2_app_kit::NSWindowOrderingMode::NSWindowBelow,
            content_view.subviews().first(),
        );
    }
}
```

**Note:** The exact objc2 API may need adjustment based on the version. If `NSVisualEffectState` or `NSAutoresizingMaskOptions` are not available, use the raw integer values. This should be verified at compile time and adjusted.

- [ ] **Step 3: Call `apply_vibrancy` in the setup hook**

In the `.setup(|app| { ... })` block, after the `info!("Meridian initialized...")` line (around line 375), add:

```rust
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                apply_vibrancy(&window);
            }
```

- [ ] **Step 4: Verify it compiles on macOS**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: `Finished` with no errors. If there are type errors with objc2 APIs, adjust the feature flags or method names based on compiler output.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat: apply NSVisualEffectView sidebar vibrancy on macOS"
```

---

### Task 9: Windows — Add Mica Material

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `window-vibrancy` dependency**

In `src-tauri/Cargo.toml`, add after the `zip` dependency:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
window-vibrancy = "0.5"
```

- [ ] **Step 2: Add Mica setup function**

In `src-tauri/src/lib.rs`, add this function:

```rust
/// Apply Mica material on Windows 11. Falls back silently on older Windows.
#[cfg(target_os = "windows")]
fn apply_mica(window: &tauri::WebviewWindow) {
    use window_vibrancy::apply_mica;
    let _ = apply_mica(window, None);
}
```

- [ ] **Step 3: Call `apply_mica` in the setup hook**

In the `.setup(|app| { ... })` block, near the macOS vibrancy call, add:

```rust
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                apply_mica(&window);
            }
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors (on macOS this will skip the Windows-only code due to `cfg`)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat: apply Mica material on Windows 11"
```

---

### Task 10: Visual Polish — Shadows, Hover, Transitions

**Files:**
- Modify: `src/index.css`
- Modify: `src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Add card shadow and interaction enhancements to index.css**

In `src/index.css`, add to the `@layer base` block (after the `body` rule):

```css
  /* Card elevation */
  .card-elevated {
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  }

  /* Enhanced interaction feedback */
  button {
    transition: background-color 150ms ease, color 150ms ease, transform 100ms ease;
  }

  button:active:not(:disabled) {
    transform: scale(0.98);
  }

  /* Table row hover */
  tr {
    transition: background-color 150ms ease;
  }
```

- [ ] **Step 2: Add `card-elevated` class to table wrappers across pages**

In each page that has a table or card container (DashboardPage, CertsPage, AccessPage), add the `card-elevated` class to the outer `<div>` that wraps the `<table>` or card list. For example, in DashboardPage, the table's wrapper `<div className="bg-bg-secondary border ...">` becomes `<div className="card-elevated bg-bg-secondary border ...">`.

- [ ] **Step 3: Improve Sidebar hover transitions**

In `src/components/layout/Sidebar.tsx`, the nav button already has `transition-all duration-150`. No CSS changes needed — this step verifies existing transitions are smooth.

- [ ] **Step 3: Verify it builds**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn" | head -5`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add src/index.css
git commit -m "feat: add subtle interaction feedback (active scale, transitions)"
```

---

### Task 11: Add `.superpowers/` to `.gitignore`

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Add the entry**

Add `.superpowers/` to the end of `.gitignore`.

- [ ] **Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: add .superpowers/ to gitignore"
```

---

### Task 12: Integration Verification

- [ ] **Step 1: Full type check**

Run: `npx tsc --noEmit 2>&1 | grep -v "npm warn"`
Expected: No errors

- [ ] **Step 2: Full Rust check**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 3: Dev mode test**

Run: `npm run tauri dev`

Verify:
- No double titlebar (macOS: traffic lights in sidebar area; Windows/Linux: single native titlebar)
- Sidebar has vibrancy effect (macOS) or Mica (Windows) or solid color (Linux)
- Engine status pill and controls appear in the content toolbar (right side)
- Page title appears in the content toolbar (left side)
- All navigation works
- Dark mode toggle works from Settings page
- Language toggle works from Settings page

- [ ] **Step 4: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: integration adjustments for native desktop style"
```
