# 轻渡 · Meridian

本地 Nginx 代理管理器，提供可视化界面管理反向代理、SSL 证书和访问控制。

A local Nginx proxy manager with a GUI for managing reverse proxies, SSL certificates, and access control.

## 功能特性 / Features

- **代理管理** — HTTP/HTTPS 反向代理与 TCP/UDP 流转发，支持端口冲突检测
- **证书管理** — 本地自签证书生成，ACME DNS-01 自动申请（Let's Encrypt）
- **访问控制** — IP 黑白名单，支持排序与去重
- **配置引擎** — 自动生成 Nginx 配置，事务性写入带备份回滚
- **Nginx 生命周期** — 启动 / 停止 / 重载 / 配置测试，状态实时同步
- **系统托盘** — 关闭窗口驻留后台，右键菜单快捷操作
- **监控面板** — 代理流量与状态监控（开发中）
- **国际化** — 中文 / English 双语切换

## 技术栈 / Tech Stack

| 层级 | 技术 |
|------|------|
| 框架 | [Tauri v2](https://v2.tauri.app/) |
| 前端 | React 19 + TypeScript + Tailwind CSS 4 + Vite |
| 状态管理 | Zustand |
| 后端 | Rust |
| 数据库 | SQLite (rusqlite) |
| 代理引擎 | Nginx |

## 前置要求 / Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- Tauri v2 系统依赖 — 参见 [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

Meridian 使用应用内管理的 Nginx sidecar。开发或打包前请准备 sidecar binary：

```bash
# macOS / Linux
./scripts/prepare-nginx.sh

# Windows
.\scripts\prepare-nginx.ps1
```

## 快速开始 / Getting Started

```bash
# 安装依赖
npm install

# 开发模式（前端热重载 + Rust 编译）
npm run tauri dev

# 生产构建
npm run tauri build
```

## 项目结构 / Project Structure

```
src/                    # React 前端
├── pages/              # 页面组件 (Dashboard, ProxyForm, Certs, Access, Logs, Settings, Monitor)
├── components/         # UI 组件
├── stores/             # Zustand 状态管理
├── locales/            # i18n 翻译文件 (zh/en)
├── lib/                # API 封装
└── types/              # TypeScript 类型定义

src-tauri/              # Rust 后端
├── src/
│   ├── commands/       # Tauri IPC 命令 (30+)
│   ├── config_engine/  # Nginx 配置生成 (HTTP/Stream/Main)
│   ├── nginx_manager/  # Nginx 进程管理
│   ├── cert_manager/   # 证书生成与管理
│   ├── acme_client/    # ACME 协议客户端
│   ├── dns_provider/   # DNS API 集成 (Cloudflare, Route53, DNSPod, AliDNS)
│   ├── metrics/        # 流量指标采集与聚合
│   ├── store/          # SQLite 数据层
│   └── validators.rs   # 输入校验
└── icons/              # 应用图标

specs/                  # 功能规格文档
```

## 许可证 / License

MIT
