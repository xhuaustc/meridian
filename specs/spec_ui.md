# Spec: Desktop UI (桌面界面)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |
| 2026-04-04 | System tray: full menu (start/stop/reload/new rule/quit), close-to-tray, i18n-aware menu text, status display | Runtime feature |
| 2026-04-04 | Custom app icon: blue gradient + flowing stream lines | Branding |
| 2026-04-04 | Tray menu items dynamically enabled/disabled based on engine state | UX improvement |
| 2026-04-04 | Engine status label removed "Nginx" prefix (now "运行中"/"Running") | UX refinement |

## Feature Description

Tauri 桌面窗口应用的前端 UI 层。包含 App Shell（侧边栏 + 标题栏 + 路由）、国际化、主题切换、系统托盘、日志查看、设置管理、配置导入导出。

## Use Cases

- UC-001: 侧边栏导航在各页面间切换
- UC-002: 标题栏显示引擎状态、语言切换、主题切换
- UC-003: 切换中文/英文界面
- UC-004: 切换 Light / Dark / System 主题
- UC-005: 查看 Nginx access/error 日志
- UC-006: 在设置页面配置 Nginx 路径、导入导出配置
- UC-007: 最小化到系统托盘

## Interface Definition

### App Shell

**Layout:**
- 标题栏（48px）：Logo + 应用名 | 引擎状态 badge | 主题按钮 | 语言按钮
- 侧边栏（220px）：分组导航（代理管理 / 安全 / 系统）
- 主内容区：由路由决定

**Routes:**
| Route | Component | Description |
|-------|-----------|-------------|
| `/` | `DashboardPage` | 代理列表 + 统计 |
| `/proxy/new` | `ProxyFormPage` | 新建代理 |
| `/proxy/:id` | `ProxyFormPage` | 编辑代理 |
| `/certs` | `CertsPage` | 证书管理 |
| `/access` | `AccessPage` | 访问控制 |
| `/logs` | `LogsPage` | 日志查看 |
| `/settings` | `SettingsPage` | 设置 |

### i18n

- **Framework:** i18next + react-i18next
- **Namespace 按模块划分：**
  - `common` — 通用按钮、状态文本
  - `proxy` — 代理管理相关
  - `cert` — 证书管理相关
  - `access` — 访问控制相关
  - `logs` — 日志相关
  - `settings` — 设置相关
- **语言文件位置：** `src/locales/{lang}/{namespace}.json`
- **切换机制：** `i18next.changeLanguage(lang)` + 保存到 AppSettings
- **初始化：** 启动时从 AppSettings 读取 language，默认跟随系统 locale（fallback: en）

### Theme System

- **三种模式：** `light` / `dark` / `system`
- **实现：** Tailwind CSS `darkMode: 'class'`，通过 `<html class="dark">` 切换
- **System 模式：** 监听 `prefers-color-scheme` media query change event
- **切换按钮：** 标题栏右侧，Sun → Moon → Monitor 图标循环
- **持久化：** AppSettings key `theme`

### Logs Page

**UI 结构：**
- 顶部：Access Log / Error Log 切换 tab + 清除按钮
- 日志区域：等宽字体、深色背景终端风格
- 每行显示：时间戳 | HTTP 状态码（彩色）| 方法 | URL | upstream | 耗时

**Backend interface:**

#### `read_access_log`
- **Input:** `{ lines?: number }` (default 200)
- **Response:** `{ entries: LogEntry[] }`

#### `read_error_log`
- **Input:** `{ lines?: number }` (default 100)
- **Response:** `{ entries: string[] }`

#### `tail_log`
- **Input:** `{ log_type: "access" | "error" }`
- **Type:** Tauri event stream
- **Notes:** 使用 Tauri event system 实时推送新日志行

### Settings Page

**设置项：**
| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| language | select | system locale | 界面语言 (zh / en) |
| theme | select | system | 主题 (light / dark / system) |
| nginx_path | file path | bundled | Nginx 可执行文件路径 |
| data_dir | file path | platform default | 数据存储目录 |

**导入导出：**
- 导出按钮 → 调用 Tauri save dialog → `export_config` 命令 → 保存 JSON
- 导入按钮 → 调用 Tauri open dialog → 读取文件 → `import_config` 命令 → 确认对话框（覆盖提示）

### System Tray

- **图标：** 应用自定义图标（蓝色渐变 + 三条流动曲线，代表数据路由），SVG 源文件生成全尺寸 PNG/ICO/ICNS
- **单击：** 显示/聚焦主窗口
- **右键菜单（跟随应用语言设置，中/英文动态切换）：**
  - `● 运行中` / `● 已停止`（状态指示，不可点击）
  - ---
  - 显示窗口 / Show Window
  - ---
  - 启动 / Start（运行中时禁用）
  - 停止 / Stop（停止时禁用）
  - 重载配置 / Reload（停止时禁用）
  - ---
  - 新建规则 / New Rule（打开窗口并导航到 `/proxy/new`）
  - ---
  - 退出 / Quit（先停止 nginx 再退出）
- **菜单状态同步：** 启动/停止/重载操作后立即刷新菜单项的启用状态和文字；右键弹出时也刷新（兜底）
- **关闭窗口行为：** 拦截 `CloseRequested` 事件，隐藏窗口到托盘（应用后台运行），非退出

## Business Rules

1. **i18n 覆盖率**：所有用户可见文本必须通过 i18n key 引用，禁止硬编码中/英文
2. **语言切换即时生效**：调用 `changeLanguage` 后无需刷新页面
3. **主题切换即时生效**：切换 `<html>` class 无需刷新
4. **System 主题跟随**：操作系统切换外观时，应用实时跟随
5. **托盘图标状态**：Nginx 运行时正常图标，停止时灰色图标
6. **日志自动滚动**：新日志行追加时自动滚动到底部，用户手动上滚时暂停自动滚动
7. **导入覆盖确认**：导入配置前显示确认对话框，告知将覆盖现有配置
8. **窗口尺寸记忆**：记住上次关闭时的窗口大小和位置

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | Switch language zh → en | All UI text changes to English | |
| TP-002 | Normal | Switch language en → zh | All UI text changes to Chinese | |
| TP-003 | Normal | Switch theme to dark | UI renders in dark mode | |
| TP-004 | Normal | Switch theme to system (OS is dark) | UI renders in dark mode | |
| TP-005 | Normal | Navigate sidebar: Dashboard → Certs → Access → Logs → Settings | Each page renders correctly | |
| TP-006 | Normal | Close window (X button) → click tray icon | Window hidden then reappears on tray click | Minimize to tray |
| TP-007 | Normal | Tray right-click → "退出/Quit" | Nginx stopped, app exits | |
| TP-018 | Normal | Tray menu when nginx running | "启动/Start" disabled, "停止/Stop" and "重载/Reload" enabled, status shows "● 运行中/Running" | |
| TP-019 | Normal | Tray menu when nginx stopped | "启动/Start" enabled, "停止/Stop" and "重载/Reload" disabled, status shows "● 已停止/Stopped" | |
| TP-020 | Normal | Switch language to EN → right-click tray | All tray menu items in English | |
| TP-021 | Normal | Switch language to ZH → right-click tray | All tray menu items in Chinese | |
| TP-022 | Normal | Tray → "新建规则/New Rule" | Window shown, navigated to /proxy/new | |
| TP-023 | Normal | Tray → "启动/Start" → right-click again | Status updated to running, Start disabled | |
| TP-008 | Normal | Read access log (200 lines) | Log entries displayed in terminal style | |
| TP-009 | Normal | Tail log → new request comes in | New line appended, auto-scroll | |
| TP-010 | Normal | Export config → Import on fresh install | All rules, certs (metadata), access lists restored | |
| TP-011 | Error | Import malformed JSON file | Error toast, existing config unchanged | |
| TP-012 | Normal | Engine status badge: nginx running | Green dot + "Running" text | |
| TP-013 | Normal | Engine status badge: nginx stopped | Grey dot + "Stopped" text | |
| TP-014 | Boundary | Scroll up in log → new entries arrive | Auto-scroll paused, manual position kept | |
| TP-015 | Normal | Close window (X button) | Window hidden, tray icon remains | Minimize to tray |
| TP-016 | Normal | Restart app → check language/theme persisted | Same language and theme as before | |
| TP-017 | Combination | Dark theme + Chinese language + tray minimize + log tail | All features work simultaneously | |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| System Tray setup | `src-tauri/src/lib.rs` | `run()` setup closure | TrayIconBuilder + TrayMenuItems struct |
| Tray menu i18n | `src-tauri/src/lib.rs` | `get_language()`, `sync_tray_menu()` | Reads language from DB settings |
| Tray menu state sync | `src-tauri/src/lib.rs` | `sync_tray_menu()` | Updates enabled/disabled + status text |
| Close-to-tray | `src-tauri/src/lib.rs` | `on_window_event` handler | Intercepts CloseRequested |
| Tray → navigate | `src/App.tsx` | `window.__navigate` | Exposed for Rust `eval()` calls |
| App icon | `src-tauri/icons/icon.svg` | — | SVG source, generates all PNG/ICO/ICNS |
| Engine status label | `src/locales/zh/common.json`, `src/locales/en/common.json` | `engine.running`, `engine.stopped` | No "Nginx" prefix |
| Nginx lifecycle logging | `src-tauri/src/nginx_manager/mod.rs` | `append_to_error_log()` | Writes start/stop/reload events to error.log |
