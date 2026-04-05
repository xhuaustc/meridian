# Requirements: Local Proxy Manager

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial draft | Phase 1 clarification |

## Background & Objectives

本地代理管理器是一款跨平台桌面软件，底层基于 Nginx 引擎，为开发者提供可视化的网络代理配置能力。用户可通过简洁的桌面界面管理四层（TCP/UDP）与七层（HTTP/HTTPS）代理规则，按域名、端口、路径前缀将请求转发至内部服务，替代手动编写和管理 Nginx 配置文件的繁琐操作。

**Core Objectives:**
1. 零门槛配置 — 开发者无需了解 Nginx 配置语法即可完成代理设置
2. 高性能转发 — 底层依托 Nginx 引擎，天然支持高并发
3. 跨平台一致体验 — Windows / Linux / macOS 原生安装包，桌面窗口管理
4. 纯本地运行 — 无需网络账户，无远程依赖，数据完全本地化

## User Roles

| Role | Description |
|------|-------------|
| Developer | 本地开发者，需要将域名/端口/路径映射到本地或内网服务，单用户使用，无需认证 |

## Use Cases

### UC-001: 添加 HTTP 反向代理规则
**Actor:** Developer
**Precondition:** 软件已启动，Nginx 引擎运行中
**Flow:**
1. 用户点击「添加代理」
2. 选择代理类型：HTTP/HTTPS (Layer 7)
3. 填写：监听域名、监听端口、目标地址（host:port）、可选路径前缀
4. 配置 TLS 选项（无 / TLS 终止 / TLS 透传）
5. 保存 → 自动生成 Nginx 配置并热重载
**Postcondition:** 新代理规则生效，流量按规则转发

### UC-002: 添加 TCP/UDP 流代理规则
**Actor:** Developer
**Precondition:** 软件已启动
**Flow:**
1. 用户点击「添加代理」
2. 选择代理类型：TCP Stream / UDP Stream (Layer 4)
3. 填写：监听端口、目标地址（host:port）
4. 保存 → 自动生成 Nginx stream 配置并热重载
**Postcondition:** 新 stream 代理规则生效

### UC-003: 管理 SSL 证书
**Actor:** Developer
**Precondition:** 至少有一条 HTTPS 代理规则
**Flow:**
1. 用户进入证书管理页面
2. 可选：上传自有证书 / 生成自签名证书 / 通过 ACME (Let's Encrypt) 自动申请
3. 将证书绑定到代理规则
**Postcondition:** HTTPS 代理使用指定证书

### UC-004: 配置 IP 访问控制
**Actor:** Developer
**Precondition:** 至少有一条代理规则
**Flow:**
1. 用户创建 Access List（命名的 IP 规则集）
2. 添加 allow/deny IP 或 CIDR 规则
3. 将 Access List 绑定到代理规则
**Postcondition:** 匹配规则的请求被允许或拒绝

### UC-005: 查看代理状态与日志
**Actor:** Developer
**Flow:**
1. 主界面显示所有代理规则的运行状态（启用/禁用/错误）
2. 用户可查看单条规则的实时访问日志
3. 用户可查看 Nginx 错误日志
**Postcondition:** 用户了解代理运行状况

### UC-006: 导入/导出配置
**Actor:** Developer
**Flow:**
1. 用户选择导出 → 生成 JSON 配置文件（包含所有规则、证书引用、访问控制列表）
2. 用户选择导入 → 从 JSON 文件还原配置
**Postcondition:** 配置可跨机器迁移

### UC-007: 切换界面语言
**Actor:** Developer
**Flow:**
1. 用户在设置中切换语言（中文 / English）
2. 界面即时切换
**Postcondition:** 所有 UI 文本以所选语言显示

## Functional Requirements

### Module: Proxy Management (代理管理)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-001 | 支持 Layer 7 反向代理（HTTP/HTTPS），可按域名 + 端口 + 路径前缀路由 | Must |
| FR-002 | 支持 Layer 4 流代理（TCP/UDP Stream），按端口转发。注：Windows 平台 UDP stream 受限于 Nginx 官方构建，标注为 Linux/macOS only | Must |
| FR-003 | 每条代理规则可独立启用/禁用，无需删除 | Must |
| FR-004 | 代理规则变更后自动生成 Nginx 配置并热重载（nginx -s reload） | Must |
| FR-005 | 支持配置自定义 HTTP 头（添加/修改/删除请求头与响应头） | Should |
| FR-006 | 支持 WebSocket 代理（upgrade 连接） | Should |
| FR-007 | 支持配置多个 upstream 目标（简单负载均衡：轮询） | Could |

### Module: TLS / Certificate Management (证书管理)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-010 | 支持 TLS 终止（软件持有证书，解密后转发明文到 upstream） | Must |
| FR-011 | 支持 TLS 透传（SNI-based passthrough，不解密，直接转发到 upstream） | Must |
| FR-012 | 支持上传自有 SSL 证书（cert + key 文件） | Must |
| FR-013 | 支持一键生成自签名证书（用于本地开发） | Must |
| FR-014 | 支持 ACME 自动证书申请（Let's Encrypt），含自动续期 | Should |
| FR-015 | 证书到期提醒（到期前 30 天在界面提示） | Should |

### Module: Access Control (访问控制)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-020 | 支持创建命名的 Access List（IP 访问控制规则集） | Must |
| FR-021 | Access List 支持 allow/deny 规则，支持单 IP 和 CIDR 段 | Must |
| FR-022 | Access List 可绑定到任意代理规则 | Must |
| FR-023 | 规则按顺序匹配，支持默认 allow/deny 策略 | Must |

### Module: Nginx Engine Management (引擎管理)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-030 | 软件内嵌或自动管理 Nginx 进程的生命周期（启动/停止/重载） | Must |
| FR-031 | 配置变更时先验证（nginx -t）再重载，验证失败需回滚并提示用户 | Must |
| FR-032 | 显示 Nginx 引擎运行状态（运行中/已停止/异常） | Must |
| FR-033 | 支持查看 Nginx access log 和 error log | Should |
| FR-034 | 上游不可达时显示自定义 502 错误页面（品牌化，含重试按钮） | Should |
| FR-035 | 支持配置工作进程数（默认 2，支持 auto），用于性能调优 | Should |

### Module: Configuration & Data (配置与数据)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-040 | 使用 SQLite 存储所有配置数据（代理规则、证书元数据、访问控制列表） | Must |
| FR-041 | 支持配置导入/导出为 JSON 文件 | Should |
| FR-042 | 应用启动时自动检查数据库迁移（schema migration） | Must |

### Module: Desktop UI (桌面界面)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-050 | 原生桌面窗口应用（非 Web 管理界面） | Must |
| FR-051 | 主界面以列表/卡片展示所有代理规则及其状态 | Must |
| FR-052 | 支持中文/英文界面切换（i18n） | Must |
| FR-053 | 系统托盘支持 — 最小化到托盘，后台运行 | Should |
| FR-054 | 深色/浅色主题切换 | Should |
| FR-055 | 代理规则搜索与筛选 | Should |

### Module: Installation & Distribution (安装与分发)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-060 | 提供 Windows 安装包（.exe / .msi） | Must |
| FR-061 | 提供 Linux 安装包（.deb + .AppImage） | Must |
| FR-062 | 提供 macOS 安装包（.dmg） | Must |
| FR-063 | 安装包内嵌或自动安装 Nginx 依赖 | Must |

## Non-Functional Requirements

| ID | Category | Requirement |
|----|----------|-------------|
| NFR-001 | Performance | 代理转发性能依托 Nginx 引擎，不低于原生 Nginx 性能的 95% |
| NFR-002 | Startup | 应用冷启动到可用状态 < 3 秒 |
| NFR-003 | Memory | 管理界面空闲时内存占用 < 150MB（不含 Nginx 进程） |
| NFR-004 | Reliability | 配置变更失败时自动回滚，不中断现有代理服务 |
| NFR-005 | Data Safety | SQLite 数据库在每次 schema 变更前自动备份 |
| NFR-006 | Cross-platform | Windows 10+, Ubuntu 20.04+/Debian 11+, macOS 12+。Windows 上所有子进程使用 CREATE_NO_WINDOW 防止控制台窗口闪烁 |
| NFR-007 | i18n | UI 文本 100% 覆盖中文和英文，支持运行时切换 |
| NFR-008 | Accessibility | 键盘可完整操作所有核心功能 |

## Tech Stack (Recommended)

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Desktop Framework | **Tauri v2** | Rust 后端 + Web 前端，轻量原生打包，跨平台 |
| Frontend | **React + TypeScript** | 生态成熟，组件库丰富 |
| UI Components | **Shadcn/ui + Tailwind CSS** | 现代设计语言，高度可定制 |
| Backend (Tauri) | **Rust** | 进程管理、文件操作、SQLite 交互 |
| Database | **SQLite** (via rusqlite) | 轻量本地存储 |
| Proxy Engine | **Nginx** | 成熟高性能，内嵌分发 |
| i18n | **i18next** | 前端国际化标准方案 |
| Packaging | Tauri bundler | 原生生成 .exe/.deb/.dmg |

## Out of Scope

- Web 管理界面（纯桌面应用）
- 用户账户与认证系统
- 多用户 / 远程访问
- 容器化部署（Docker）
- 内置 CDN / 缓存功能
- API Gateway 高级功能（限流、熔断、鉴权网关）
- Nginx 以外的代理引擎支持

## Product Name

- **中文名：轻渡** (Qīngdù) — 轻舟已过万重山，寓意代理转发无感高效、流量轻盈通行
- **英文名：Meridian** — 子午线，连接南北的通道，寓意网络路径的精确引导
- **全称：轻渡 · Meridian**
- **气质：** 诗意 + 轻量 + 精准
