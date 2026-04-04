# Task Breakdown: 轻渡 · Meridian

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial task breakdown | Phase 2c |

## Tasks

| Task ID | Name | Module | Deps | Related Spec | Acceptance Criteria | AI-Auto | Status |
|---------|------|--------|------|--------------|---------------------|---------|--------|
| TASK-001 | Tauri 项目脚手架 | Infra | — | — | `cargo tauri dev` 成功启动空窗口；React + TS + Vite + Tailwind + Shadcn/ui 初始化完成 | Yes | Pending |
| TASK-002 | SQLite 数据层 + Migration | Store | TASK-001 | spec_store.md | 所有表创建成功；CRUD 函数通过单元测试；备份/还原正常 | Yes | Pending |
| TASK-003 | Tauri Commands 框架 | Commands | TASK-001 | — | 前端可通过 invoke 调用后端命令并收到响应；错误格式统一 | Yes | Pending |
| TASK-004 | i18n 框架搭建 | Frontend | TASK-001 | spec_ui.md | 中英文切换正常；所有 key 有 zh/en 翻译 | Yes | Pending |
| TASK-005 | 主题系统（Light/Dark/System） | Frontend | TASK-004 | spec_ui.md | 三种主题模式正常切换；偏好持久化到 SQLite | Yes | Pending |
| TASK-006 | App Shell（侧边栏 + 路由 + 标题栏） | Frontend | TASK-004, TASK-005 | spec_ui.md | 侧边栏导航、页面路由、引擎状态 badge、主题/语言切换均正常 | Yes | Pending |
| TASK-007 | Nginx Manager（进程生命周期） | NginxMgr | TASK-001 | spec_nginx_manager.md | start/stop/reload/test 正常工作；状态检测准确 | Yes | Pending |
| TASK-008 | Config Engine（配置生成） | ConfigEngine | TASK-002 | spec_config_engine.md | 从 DB 规则生成正确的 nginx.conf / conf.d / stream.d；多域名同端口聚合正确 | Yes | Pending |
| TASK-009 | 端口冲突检测 | ConfigEngine | TASK-008 | spec_config_engine.md | 5 种冲突场景检测正确；前端表单实时校验 | Yes | Pending |
| TASK-010 | 代理规则 CRUD（后端） | Commands | TASK-002, TASK-008, TASK-007 | spec_proxy.md | create/read/update/delete/toggle 命令正常；变更后自动生成配置并 reload | Yes | Pending |
| TASK-011 | Dashboard 页面（代理列表 + 统计） | Frontend | TASK-006, TASK-010 | spec_proxy.md | 规则列表、统计卡片、搜索筛选、开关切换均正常 | Yes | Pending |
| TASK-012 | Proxy Form 页面（创建/编辑） | Frontend | TASK-011, TASK-009 | spec_proxy.md | 表单创建/编辑 L4/L7 规则正常；类型切换隐藏无关字段；端口冲突即时提示 | Yes | Pending |
| TASK-013 | Certificate Manager（后端） | CertMgr | TASK-002 | spec_cert.md | 自签名生成、导入、过期检查正常；私钥文件权限 0600 | Yes | Pending |
| TASK-014 | ACME 自动证书 | CertMgr | TASK-013 | spec_cert.md | Let's Encrypt 申请 + 自动续期正常 | Yes | Pending |
| TASK-015 | 证书管理页面 | Frontend | TASK-006, TASK-013 | spec_cert.md | 证书列表、上传、生成自签名、到期提醒显示正常 | Yes | Pending |
| TASK-016 | Access List CRUD（后端） | Commands | TASK-002 | spec_access.md | Access List + Rule CRUD 正常；生成正确的 nginx allow/deny 指令 | Yes | Pending |
| TASK-017 | 访问控制页面 | Frontend | TASK-006, TASK-016 | spec_access.md | 列表创建/编辑、IP 规则增删改、绑定代理规则正常 | Yes | Pending |
| TASK-018 | 日志查看（后端 + 前端） | Logs | TASK-006, TASK-007 | spec_ui.md | 读取 access/error log、实时 tail、按类型切换正常 | Yes | Pending |
| TASK-019 | 设置页面 + 导入导出 | Settings | TASK-006, TASK-002 | spec_ui.md | 语言/主题/Nginx 路径设置正常；JSON 导入导出正常 | Yes | Pending |
| TASK-020 | 系统托盘 | Frontend | TASK-006, TASK-007 | spec_ui.md | 最小化到托盘、右键菜单（显示/退出）、托盘图标状态指示 | Yes | Pending |
| TASK-021 | Nginx 内嵌 + 首次运行初始化 | Infra | TASK-007, TASK-008 | spec_nginx_manager.md | 各平台 Nginx binary 正确加载；首次运行自动创建数据目录和默认配置 | Yes | Pending |
| TASK-022 | 跨平台打包 + CI | Infra | TASK-021 | — | GitHub Actions CI 三平台构建通过；输出 .exe/.deb/.dmg | No | Pending |
| TASK-023 | 应用图标设计 + 集成 | Infra | TASK-001 | — | 应用图标在三平台正确显示（标题栏、托盘、安装包） | No | Pending |

Status values: `Pending` | `In Progress` | `Done` | `Blocked [reason]`
