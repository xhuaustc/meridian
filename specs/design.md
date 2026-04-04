# Technical Design: 轻渡 · Meridian

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial design | Phase 2a |
| 2026-04-04 | Add port conflict detection, multi-domain per port, CI build matrix, theme toggle, cert permissions | Gate 2 feedback |

## Architecture Overview

轻渡 · Meridian 采用 **Tauri v2** 桌面架构：Rust 后端负责 Nginx 进程管理、配置生成、SQLite 数据操作；React + TypeScript 前端负责用户界面渲染。两层通过 Tauri IPC (command) 通信。

```
┌─────────────────────────────────────────────────────┐
│                   Desktop Window                     │
│  ┌───────────────────────────────────────────────┐  │
│  │           React + TypeScript Frontend          │  │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────────┐  │  │
│  │  │ Proxy   │ │ Cert     │ │ Access List   │  │  │
│  │  │ Manager │ │ Manager  │ │ Manager       │  │  │
│  │  ├─────────┤ ├──────────┤ ├───────────────┤  │  │
│  │  │ Log     │ │ Settings │ │ Engine Status │  │  │
│  │  │ Viewer  │ │          │ │               │  │  │
│  │  └─────────┘ └──────────┘ └───────────────┘  │  │
│  └──────────────────┬────────────────────────────┘  │
│                     │ Tauri IPC (invoke)             │
│  ┌──────────────────▼────────────────────────────┐  │
│  │             Rust Backend (Tauri)               │  │
│  │  ┌───────────────┐  ┌──────────────────────┐  │  │
│  │  │ Config Engine │  │ Nginx Manager        │  │  │
│  │  │ (generate +   │  │ (lifecycle + reload) │  │  │
│  │  │  validate)    │  │                      │  │  │
│  │  └───────┬───────┘  └──────────┬───────────┘  │  │
│  │          │                     │               │  │
│  │  ┌───────▼───────┐  ┌─────────▼───────────┐  │  │
│  │  │ SQLite Store  │  │ Certificate Manager │  │  │
│  │  │ (rusqlite)    │  │ (files + ACME)      │  │  │
│  │  └───────────────┘  └─────────────────────┘  │  │
│  └───────────────────────────────────────────────┘  │
│                     │                                │
│        ┌────────────▼────────────────┐              │
│        │    Nginx Process (child)    │              │
│        │  ┌────────┐  ┌───────────┐  │              │
│        │  │ HTTP   │  │ Stream    │  │              │
│        │  │ server │  │ (TCP/UDP) │  │              │
│        │  └────────┘  └───────────┘  │              │
│        └─────────────────────────────┘              │
└─────────────────────────────────────────────────────┘
```

**Tech Stack:**
- Desktop Framework: Tauri v2
- Frontend: React 18 + TypeScript 5 + Vite
- UI Components: Shadcn/ui + Tailwind CSS 4
- Backend: Rust (Tauri commands)
- Database: SQLite via rusqlite (with migrations via refinery)
- Proxy Engine: Nginx (bundled binary)
- i18n: i18next + react-i18next
- State Management: Zustand
- Packaging: Tauri bundler → .exe/.deb/.dmg

## Module Breakdown

### 1. Frontend: UI Layer (`src/`)

**Responsibility:** 渲染用户界面，处理用户交互，通过 Tauri IPC 调用后端命令
**Key interfaces:**
- Tauri `invoke()` calls → Rust commands
- React Router for navigation
- Zustand stores for local state
**Dependencies:** Tauri IPC, i18next

**Pages/Views:**
| Page | Route | Description |
|------|-------|-------------|
| Dashboard | `/` | 代理规则列表 + 引擎状态概览 |
| Proxy Form | `/proxy/new`, `/proxy/:id` | 创建/编辑代理规则（L4/L7 共用表单，按类型切换字段） |
| Certificates | `/certs` | 证书列表、上传、生成自签名、ACME 申请 |
| Access Lists | `/access` | IP 访问控制规则集管理 |
| Logs | `/logs` | Nginx access/error 日志查看 |
| Settings | `/settings` | 语言切换、主题、Nginx 路径配置、导入导出 |

**主题切换：**
- 标题栏右侧放置主题切换按钮（sun/moon icon），与语言切换按钮并排
- 支持三种模式：Light / Dark / System（跟随系统）
- 使用 Tailwind CSS `dark:` 变体实现，通过 `<html class="dark">` 切换
- 主题偏好存储在 AppSettings（key: `theme`）
- 应用启动时读取偏好，若为 `system` 则通过 `prefers-color-scheme` media query 检测

### 2. Backend: Config Engine (`src-tauri/src/config_engine/`)

**Responsibility:** 将 SQLite 中的代理规则转换为 Nginx 配置文件
**Key interfaces:**
- `generate_http_config(rules: &[HttpProxyRule]) -> Result<String>` — 生成 http server blocks
- `generate_stream_config(rules: &[StreamProxyRule]) -> Result<String>` — 生成 stream blocks
- `generate_main_config(settings: &NginxSettings) -> Result<String>` — 生成 nginx.conf 主配置
- `write_configs(output_dir: &Path) -> Result<()>` — 写入配置文件到磁盘
- `validate_port_conflicts(rules: &[ProxyRule]) -> Result<Vec<PortConflict>>` — 端口冲突检测
**Dependencies:** SQLite Store

**端口冲突检测规则：**

同一个端口在不同代理类型下的冲突语义不同：

| 场景 | 是否冲突 | 原因 |
|------|---------|------|
| HTTP/HTTPS 同端口 + 不同域名 | ✅ 允许 | Nginx 按 `server_name` 虚拟主机路由 |
| HTTP/HTTPS 同端口 + 同域名 + 不同 path_prefix | ✅ 允许 | Nginx 按 `location` 路径匹配 |
| HTTP/HTTPS 同端口 + 同域名 + 同 path_prefix | ❌ 冲突 | |
| TCP/UDP Stream 同端口 | ❌ 冲突 | Stream 无域名路由，端口独占 |
| HTTP/HTTPS 与 Stream 同端口 | ❌ 冲突 | http 和 stream 不能共用端口 |

冲突检测在以下时机触发：
1. **保存规则时** — 前端即时校验 + 后端二次校验
2. **生成配置前** — Config Engine 最终校验，冲突则拒绝生成

冲突时 UI 行为：
- 表单保存前，端口输入框失焦时即触发检测
- 冲突时显示 inline warning：「端口 443 已被 "Frontend Dev Server" 使用（TCP Stream 端口独占）」
- HTTP/HTTPS 同端口不同域名时，显示 info 提示：「端口 80 已有 2 条规则，将按域名路由」

**多域名共享端口的 Nginx 配置生成策略：**

同端口的 HTTP/HTTPS 规则聚合到同一个配置文件（按端口分组），每个域名生成一个 `server` block：

```nginx
# conf.d/port_443.conf (自动聚合)
server {
    listen 443 ssl;
    server_name app.local;
    ssl_certificate     certs/cert_001.pem;
    ssl_certificate_key certs/cert_001.key;
    location / { proxy_pass http://127.0.0.1:3000; }
}

server {
    listen 443 ssl;
    server_name api.local;
    ssl_certificate     certs/cert_002.pem;
    ssl_certificate_key certs/cert_002.key;
    location /v1 { proxy_pass http://127.0.0.1:8080; }
    location /graphql { proxy_pass http://127.0.0.1:4000; }
}
```

**配置输出结构：**
```
data/
├── nginx/
│   ├── nginx.conf              ← 主配置（worker_processes, events, includes）
│   ├── conf.d/
│   │   ├── http_proxy_001.conf ← 每条 L7 规则一个文件
│   │   ├── http_proxy_002.conf
│   │   └── ...
│   ├── stream.d/
│   │   ├── stream_001.conf     ← 每条 L4 规则一个文件
│   │   └── ...
│   ├── certs/                  ← 证书文件存储
│   │   ├── cert_001.pem
│   │   └── cert_001.key
│   └── logs/
│       ├── access.log
│       └── error.log
├── meridian.db                 ← SQLite 数据库
└── backups/                    ← 数据库备份
```

### 3. Backend: Nginx Manager (`src-tauri/src/nginx_manager/`)

**Responsibility:** 管理 Nginx 子进程的完整生命周期
**Key interfaces:**
- `start() -> Result<()>` — 启动 Nginx 进程
- `stop() -> Result<()>` — 优雅停止
- `reload() -> Result<()>` — 热重载（先 test，再 reload）
- `test_config() -> Result<TestResult>` — 验证配置（nginx -t）
- `status() -> EngineStatus` — 获取运行状态
- `get_bundled_nginx_path() -> PathBuf` — 获取内嵌 Nginx 路径
**Dependencies:** Config Engine (for config file paths)

**生命周期状态机：**
```
         start()           reload()
Stopped ────────► Running ◄────────► Running (new config)
   ▲                │                    │
   │     stop()     │    test fail       │
   └────────────────┘    ────────► Running (old config, error reported)
                         (rollback)
```

### 4. Backend: SQLite Store (`src-tauri/src/store/`)

**Responsibility:** 所有配置数据的持久化存储与查询
**Key interfaces:**
- `ProxyRuleRepo` — CRUD for proxy rules
- `CertificateRepo` — CRUD for certificates
- `AccessListRepo` — CRUD for access lists + rules
- `SettingsRepo` — app settings (language, theme, nginx path)
- `migrate() -> Result<()>` — 运行 schema 迁移
- `backup(path: &Path) -> Result<()>` — 数据库备份
- `export_json() -> Result<String>` / `import_json(data: &str) -> Result<()>`
**Dependencies:** None (底层模块)

### 5. Backend: Certificate Manager (`src-tauri/src/cert_manager/`)

**Responsibility:** SSL 证书的生成、存储、ACME 申请与续期管理
**Key interfaces:**
- `generate_self_signed(domain: &str) -> Result<CertInfo>` — 生成自签名证书
- `import_certificate(cert_pem: &[u8], key_pem: &[u8]) -> Result<CertInfo>` — 导入外部证书
- `request_acme(domain: &str) -> Result<CertInfo>` — ACME 证书申请
- `check_expiry() -> Vec<ExpiryWarning>` — 检查证书过期
- `renew_if_needed() -> Result<Vec<RenewalResult>>` — 自动续期
**Dependencies:** SQLite Store, file system (cert storage)

**证书文件权限：**
- 证书私钥文件写入后立即设置权限为 `0600`（owner read/write only）
- Linux/macOS：使用 `std::fs::set_permissions` 设置 Unix mode
- Windows：使用 ACL 限制为当前用户访问
- 证书目录（`data/nginx/certs/`）权限设置为 `0700`

### 6. Backend: Tauri Commands (`src-tauri/src/commands/`)

**Responsibility:** 暴露 Rust 函数为 Tauri IPC 命令，供前端调用
**Key interfaces:** 每个命令对应一个 `#[tauri::command]` 函数

| Command Group | Commands |
|--------------|----------|
| `proxy` | `list_proxies`, `get_proxy`, `create_proxy`, `update_proxy`, `delete_proxy`, `toggle_proxy`, `check_port_conflict` |
| `cert` | `list_certs`, `import_cert`, `generate_self_signed`, `request_acme`, `delete_cert` |
| `access` | `list_access_lists`, `create_access_list`, `update_access_list`, `delete_access_list` |
| `engine` | `get_engine_status`, `start_engine`, `stop_engine`, `restart_engine` |
| `logs` | `read_access_log`, `read_error_log`, `tail_log` |
| `settings` | `get_settings`, `update_settings`, `export_config`, `import_config` |

**Dependencies:** All backend modules

## Data Models

### ProxyRule (代理规则)
| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| id | TEXT | PK, UUID | Auto-generated |
| name | TEXT | NOT NULL | 用户定义的规则名称 |
| proxy_type | TEXT | NOT NULL, CHECK(IN 'http','stream_tcp','stream_udp') | 代理类型 |
| enabled | INTEGER | NOT NULL, DEFAULT 1 | 0=禁用, 1=启用 |
| listen_port | INTEGER | NOT NULL | 监听端口 |
| listen_host | TEXT | DEFAULT '0.0.0.0' | 监听地址 |
| domain | TEXT | NULLABLE | 域名（仅 L7） |
| path_prefix | TEXT | NULLABLE | 路径前缀（仅 L7），如 `/api` |
| upstream_host | TEXT | NOT NULL | 目标地址 |
| upstream_port | INTEGER | NOT NULL | 目标端口 |
| tls_mode | TEXT | DEFAULT 'none', CHECK(IN 'none','terminate','passthrough') | TLS 模式 |
| certificate_id | TEXT | FK → Certificate.id, NULLABLE | 绑定证书（terminate 模式） |
| access_list_id | TEXT | FK → AccessList.id, NULLABLE | 绑定访问控制列表 |
| websocket | INTEGER | DEFAULT 0 | 是否启用 WebSocket（仅 L7） |
| custom_headers | TEXT | NULLABLE | JSON: `[{"op":"add","name":"X-Foo","value":"bar"}]` |
| sort_order | INTEGER | DEFAULT 0 | 列表排序 |
| created_at | TEXT | NOT NULL | ISO 8601 |
| updated_at | TEXT | NOT NULL | ISO 8601 |

### Certificate (证书)
| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| id | TEXT | PK, UUID | |
| name | TEXT | NOT NULL | 证书名称 |
| domain | TEXT | NOT NULL | 关联域名 |
| cert_path | TEXT | NOT NULL | 证书文件路径（相对 data/nginx/certs/） |
| key_path | TEXT | NOT NULL | 私钥文件路径 |
| source | TEXT | NOT NULL, CHECK(IN 'upload','self_signed','acme') | 证书来源 |
| expires_at | TEXT | NOT NULL | 过期时间 ISO 8601 |
| auto_renew | INTEGER | DEFAULT 0 | 是否自动续期（仅 ACME） |
| created_at | TEXT | NOT NULL | |

### AccessList (访问控制列表)
| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| id | TEXT | PK, UUID | |
| name | TEXT | NOT NULL, UNIQUE | 列表名称 |
| default_policy | TEXT | NOT NULL, CHECK(IN 'allow','deny') | 默认策略 |
| created_at | TEXT | NOT NULL | |

### AccessRule (访问规则)
| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| id | TEXT | PK, UUID | |
| access_list_id | TEXT | FK → AccessList.id, NOT NULL | |
| action | TEXT | NOT NULL, CHECK(IN 'allow','deny') | |
| ip_cidr | TEXT | NOT NULL | 单 IP 或 CIDR，如 `192.168.1.0/24` |
| sort_order | INTEGER | DEFAULT 0 | 匹配顺序（越小越先匹配） |
| created_at | TEXT | NOT NULL | |

### AppSettings (应用设置)
| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| key | TEXT | PK | 设置键名 |
| value | TEXT | NOT NULL | 设置值（JSON 序列化） |

**预定义 key：**
- `language` — `"zh"` / `"en"`
- `theme` — `"light"` / `"dark"` / `"system"`
- `nginx_path` — Nginx 可执行文件路径（默认：bundled）
- `data_dir` — 数据目录路径

## Key Interfaces & Interaction Flows

### Flow 1: 创建代理规则
```
User ──[fill form]──► Frontend ──[invoke create_proxy]──► Tauri Command
    ──► ProxyRuleRepo.insert()
    ──► ConfigEngine.generate_http_config() / generate_stream_config()
    ──► ConfigEngine.write_configs()
    ──► NginxManager.test_config()
        ├─ OK ──► NginxManager.reload() ──► return Success
        └─ Fail ──► rollback config files ──► return Error(validation message)
```

### Flow 2: 应用启动
```
App Start
  ├─► SQLite Store.migrate()          — 检查 & 执行数据库迁移
  ├─► SettingsRepo.load()             — 加载语言、主题等设置
  ├─► ConfigEngine.write_configs()    — 从 DB 重新生成所有 Nginx 配置
  ├─► NginxManager.start()            — 启动 Nginx 子进程
  ├─► CertManager.check_expiry()      — 检查证书过期
  └─► Frontend render                 — 渲染主界面
```

### Flow 3: 配置变更回滚
```
User saves change
  ──► ConfigEngine.write_configs()    — 写入新配置
  ──► NginxManager.test_config()
      ├─ OK ──► NginxManager.reload()
      └─ Fail:
          ├─► ConfigEngine.restore_previous_configs()  — 还原上一版配置文件
          ├─► NginxManager.test_config()               — 验证还原后配置
          └─► return Error with nginx -t output to UI
```

## Cross-Cutting Concerns

### Error Handling
- **Classification:**
  - `ConfigError` — 配置生成/验证错误（可恢复，回滚）
  - `EngineError` — Nginx 进程错误（需用户干预）
  - `StoreError` — 数据库操作错误
  - `CertError` — 证书操作错误
  - `ValidationError` — 用户输入校验错误
- **Propagation:** Rust errors 通过 Tauri command 返回前端，前端显示 toast 或 inline error
- **Rollback:** 配置变更失败时自动回滚到上一版 Nginx 配置文件

### Observability
- **Logging:** Rust 后端使用 `tracing` crate，日志写入 `data/app.log`
- **Nginx logs:** access.log + error.log 存储在 `data/nginx/logs/`

### Deployment & Configuration
- **Bundled Nginx:** 每个平台安装包内嵌对应平台的 Nginx 二进制
  - Windows: `resources/nginx/nginx.exe` (官方 Windows 构建)
  - Linux: `resources/nginx/nginx` (静态编译，含 stream 模块)
  - macOS: `resources/nginx/nginx` (静态编译，含 stream 模块)
- **Data directory:**
  - Windows: `%APPDATA%/meridian/`
  - Linux: `~/.local/share/meridian/`
  - macOS: `~/Library/Application Support/meridian/`
- **First run:** 自动创建数据目录、初始化 SQLite、生成默认 nginx.conf

### CI Build Matrix (Nginx + Application)

**Nginx 预编译构建：**

| Platform | Arch | Nginx Source | Build Method | Stream Module |
|----------|------|-------------|-------------|---------------|
| Windows | x86_64 | 官方 Windows 构建 | 直接下载 | TCP only |
| Linux | x86_64 | 源码编译 | Docker (Alpine) + `--with-stream` | TCP + UDP |
| Linux | aarch64 | 源码编译 | Docker (Alpine, cross) + `--with-stream` | TCP + UDP |
| macOS | x86_64 | 源码编译 | Homebrew tap / 源码 + `--with-stream` | TCP + UDP |
| macOS | aarch64 (Apple Silicon) | 源码编译 | 源码 + `--with-stream` | TCP + UDP |

**Application 构建：**

| Platform | Arch | Tauri Target | Output |
|----------|------|-------------|--------|
| Windows | x86_64 | `tauri build --target x86_64-pc-windows-msvc` | `.exe` / `.msi` |
| Linux | x86_64 | `tauri build --target x86_64-unknown-linux-gnu` | `.deb` + `.AppImage` |
| Linux | aarch64 | `tauri build --target aarch64-unknown-linux-gnu` | `.deb` + `.AppImage` |
| macOS | universal | `tauri build --target universal-apple-darwin` | `.dmg` (universal binary) |

**CI Pipeline (GitHub Actions):**
1. `nginx-build` job — 预编译各平台 Nginx，缓存为 artifacts
2. `app-build` job (depends on nginx-build) — 将 Nginx artifacts 放入 `resources/`，执行 `tauri build`
3. `release` job — 上传到 GitHub Releases

## NFR Fulfillment Matrix

| NFR ID | Requirement | Design Response | Verification Method |
|--------|-------------|-----------------|---------------------|
| NFR-001 | 代理性能不低于原生 Nginx 95% | 直接使用 Nginx 进程处理流量，管理层不在数据路径上 | wrk/ab benchmark 对比 |
| NFR-002 | 冷启动 < 3s | Tauri 轻量运行时 + SQLite 本地读取 | 计时测试 |
| NFR-003 | 空闲内存 < 150MB | Tauri WebView 替代 Electron，Rust 后端零 GC | 进程监控 |
| NFR-004 | 配置失败自动回滚 | nginx -t → reload 两步验证，失败还原配置文件 | 故意写入错误配置测试 |
| NFR-005 | 迁移前自动备份 | refinery migration hook 触发 SQLite 文件复制 | 检查 backups/ 目录 |
| NFR-006 | 跨平台支持 | Tauri v2 原生支持三平台打包 | CI 三平台构建 |
| NFR-007 | i18n 100% 覆盖 | i18next namespace 按模块组织，CI 检查缺失 key | 脚本对比 zh/en JSON |
| NFR-008 | 键盘可操作 | Shadcn/ui 内置 a11y，补充 Tab 顺序和快捷键 | 键盘遍历测试 |

## Architecture Decisions

### ADR-001: Tauri v2 vs Electron
- **Context:** 需要跨平台桌面应用框架
- **Options:** Tauri v2 / Electron / Wails (Go)
- **Decision:** Tauri v2
- **Consequences:**
  - ✅ 打包体积小（~10MB vs Electron ~150MB）
  - ✅ 内存占用低（系统 WebView vs 内嵌 Chromium）
  - ✅ Rust 后端天然适合进程管理和文件操作
  - ⚠️ Tauri v2 生态比 Electron 年轻，部分功能需自行实现
  - ⚠️ 团队需要 Rust 能力

### ADR-002: 每条规则独立配置文件 vs 单一配置文件
- **Context:** Nginx 配置文件组织方式
- **Options:** 所有规则写入一个文件 / 每条规则一个文件通过 include 引入
- **Decision:** 每条规则独立文件，主配置 include `conf.d/*.conf` 和 `stream.d/*.conf`
- **Consequences:**
  - ✅ 单条规则变更不影响其他文件
  - ✅ 调试时可直接查看单个规则文件
  - ✅ 禁用规则只需移除 include 或重命名文件
  - ⚠️ 大量规则时文件数较多（可接受，本地开发场景规则数有限）

### ADR-003: 内嵌 Nginx vs 依赖系统安装
- **Context:** Nginx 引擎如何获取
- **Options:** 安装包内嵌 / 要求用户预装 / 自动下载
- **Decision:** 安装包内嵌预编译 Nginx
- **Consequences:**
  - ✅ 开箱即用，零配置
  - ✅ 控制 Nginx 版本和编译参数（确保含 stream 模块）
  - ⚠️ 安装包体积增大（约 +5MB）
  - ⚠️ 需维护三平台 Nginx 构建

### ADR-004: SQLite vs 配置文件 (YAML/JSON)
- **Context:** 配置数据存储方式
- **Options:** SQLite / YAML files / JSON files
- **Decision:** SQLite
- **Consequences:**
  - ✅ 结构化查询，支持事务
  - ✅ Schema migration 工具支持
  - ✅ 单文件备份简单
  - ⚠️ 用户无法直接手编配置文件（通过 export/import JSON 补偿）

## Tech Choices & Trade-offs

| Decision | Chosen | Alternative | Rationale |
|----------|--------|-------------|-----------|
| Desktop framework | Tauri v2 | Electron | 体积小、内存低、Rust 后端适合系统操作 |
| Frontend | React + TS | Vue, Svelte | 生态最成熟，Shadcn/ui 组件库仅支持 React |
| UI library | Shadcn/ui | Ant Design, MUI | 高度可定制、现代设计、不臃肿 |
| CSS | Tailwind CSS 4 | CSS Modules | 与 Shadcn/ui 配套，开发效率高 |
| State mgmt | Zustand | Redux, Jotai | 轻量、TypeScript 友好、无 boilerplate |
| DB | SQLite (rusqlite) | sled, JSON files | 成熟可靠、SQL 查询灵活 |
| Migration | refinery | sqlx migrate | 纯 Rust、支持嵌入式迁移 |
| i18n | i18next | FormatJS | 生态最大、React 集成好 |
| Proxy engine | Nginx | HAProxy, Caddy | 最成熟、Windows 有官方构建 |
