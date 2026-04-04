# Design: Hosts Management (本地 Hosts 管理)

## Changelog

| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial design | Brainstorming session |

## Overview

为 Meridian 增加本地 hosts 文件管理功能，补齐"配置代理 → 配置证书 → 配置解析"的工作流闭环。用户可以在 Meridian 中管理 hosts 条目，将自定义域名解析到本机或局域网 IP，配合代理规则使用。

## Scope

- **In scope:** 本地 hosts 文件托管区块管理、独立管理页面、与代理表单的智能提示联动、跨平台提权写入
- **Out of scope:** 远程 DNS provider 记录管理、本地 DNS 服务（dnsmasq 等）、hosts 文件监听/双向同步

## Design Decisions

| 决策 | 选项 | 选择 | 理由 |
|------|------|------|------|
| 使用场景 | 开发环境 / 本地+局域网 / 通用编辑器 | 本地+局域网 | 目标 IP 不固定，需要支持局域网 IP |
| 与代理的关联方式 | 强绑定 / 弱关联 / 完全独立 | 弱关联 | hosts 和代理粒度不同，弱关联通过智能提示保持便捷性，同时保留独立管理灵活性 |
| hosts 文件修改方式 | 直接读写 / 托管区块 / 独立 DNS 服务 | 托管区块 | 只操作 Meridian 标记区块，不动用户手写条目，安全可控 |
| 权限提升方式 | 启动时获取 / 按需请求 / helper daemon | 按需请求 + 批量合并 | 最小权限原则，每次全量重建区块天然只弹一次密码 |
| 导航位置 | 一级入口 / 二级分组 / Settings 子页面 | 侧边栏一级入口 | 与 Certs、Access 同级，使用频率够高，方便智能提示跳转 |
| 技术方案 | 纯前端驱动 / 文件监听双向同步 / 只管DB按需刷写 | 纯前端驱动 | 与现有模块模式一致，复杂度适中 |

## Data Model

### HostEntry 表

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | TEXT (UUID) | Yes | 主键 |
| ip | TEXT | Yes | 目标 IP（如 `127.0.0.1`、`192.168.1.100`） |
| hostname | TEXT | Yes | 域名（如 `app.local`），唯一约束 |
| comment | TEXT | No | 备注（如"前端开发服务"） |
| enabled | BOOLEAN | Yes | 是否生效，默认 true |
| created_at | TEXT | Yes | 创建时间 |
| updated_at | TEXT | Yes | 更新时间 |

- `hostname` 加唯一约束，一个域名只能对应一个 IP
- 不存代理关联 ID——弱关联通过域名匹配实现，不建立外键

## Backend Interface

### Tauri Commands

| 命令 | 输入 | 输出 | 说明 |
|------|------|------|------|
| `list_hosts` | `{ keyword?: string }` | `{ entries: HostEntry[] }` | 列表，可按域名/IP/备注搜索 |
| `create_host` | `{ ip, hostname, comment? }` | `{ entry: HostEntry }` | 创建条目，同步写入系统 hosts |
| `update_host` | `{ id, ip?, hostname?, comment? }` | `{ entry: HostEntry }` | 更新条目，同步写入系统 hosts |
| `delete_host` | `{ id }` | `{ success: true }` | 删除条目，同步写入系统 hosts |
| `toggle_host` | `{ id, enabled }` | `{ entry: HostEntry }` | 启用/禁用，同步写入系统 hosts |
| `check_hostname_exists` | `{ hostname, exclude_id? }` | `{ exists: bool, entry?: HostEntry }` | 检查域名是否已有条目 |
| `sync_hosts_file` | `{}` | `{ success: true }` | 强制将 SQLite 数据刷写到系统 hosts 文件 |

### Error Codes

| Code | 条件 |
|------|------|
| `VALIDATION_ERROR` | IP 格式错误、hostname 格式错误 |
| `HOSTNAME_DUPLICATE` | hostname 已存在 |
| `PERMISSION_DENIED` | 提权失败（用户取消密码输入） |
| `HOSTS_FILE_ERROR` | 系统 hosts 文件读写失败 |

## Hosts File Write Logic

1. 读取系统 hosts 文件全文
2. 找到 `# >>> Meridian managed` 和 `# <<< Meridian managed` 标记
3. 替换标记之间的内容为 SQLite 中所有 `enabled=true` 的条目
4. 如果标记不存在，在文件末尾追加标记区块
5. 通过平台提权命令写回文件

生成的区块格式：

```
# >>> Meridian managed — DO NOT EDIT THIS BLOCK
127.0.0.1    app.local           # 前端服务
192.168.1.100 api.dev.com        # 后端 API
# <<< Meridian managed
```

## Cross-Platform Elevation

| 平台 | 方式 | 说明 |
|------|------|------|
| macOS | `osascript -e 'do shell script "..." with administrator privileges'` | 弹系统密码框 |
| Linux | `pkexec tee /etc/hosts` | 通过 polkit 弹密码框 |
| Windows | 直接写入 `C:\Windows\System32\drivers\etc\hosts` | 通常已有写入权限；若无则提示以管理员身份运行 |

每次操作从 SQLite 全量重建 Meridian 托管区块再一次性写入，天然只弹一次密码。

## Proxy Form Integration

### 创建代理时的智能提示

**触发条件：** 代理类型为 `http`，且 `domain` 字段非空

**流程：**

1. 保存代理规则成功后，调用 `check_hostname_exists({ hostname: domain })`
2. 如果域名没有对应的 hosts 条目，弹出 Dialog：
   > "域名 `app.local` 尚未配置本地解析，是否添加 hosts 条目？"
   >
   > IP 地址：`[127.0.0.1]`（可编辑输入框，默认 127.0.0.1）
   >
   > [添加] [跳过]
3. 用户点"添加"→ 调用 `create_host`，点"跳过"→ 不做任何操作

只在**创建**代理时提示。编辑时仅当域名发生变更才对新域名做检查。

### 删除代理时的联动提示

**流程：**

1. 删除代理前，用 `list_proxies` 获取所有代理规则，精确匹配 `domain` 字段（非模糊搜索）查询同域名的其他规则
2. 如果该域名只有这一条代理规则，且在 hosts 中有对应条目，弹出 Dialog：
   > "域名 `app.local` 没有其他代理规则在使用，是否同时删除 hosts 条目？"
   >
   > [同时删除] [保留]
3. 如果还有其他规则引用该域名，不提示

## Frontend Design

### Navigation

- 路由：`/hosts`
- 侧边栏位置：Access 之后
- 图标：`Globe`（lucide-react）

### Page Layout

与 Access 页面风格一致：

**顶部工具栏：**
- 搜索框（按域名/IP/备注过滤）
- "添加条目"按钮

**列表表格：**

| 启用 | 域名 | IP 地址 | 备注 | 操作 |
|------|------|---------|------|------|
| Toggle | `app.local` | `127.0.0.1` | 前端服务 | 编辑 / 删除 |

- Toggle 列：直接调用 `toggle_host`
- 操作列：编辑打开 Dialog，删除弹确认

**添加/编辑 Dialog：**

| 字段 | 类型 | 验证 |
|------|------|------|
| 域名 | 文本输入 | 必填，合法主机名格式，唯一性检查 |
| IP 地址 | 文本输入 | 必填，合法 IPv4/IPv6 格式 |
| 备注 | 文本输入 | 可选，最长 200 字符 |

**页面底部：**
- "同步到系统"按钮，调用 `sync_hosts_file`，用于提权失败后的手动重试或区块被破坏后的修复

### Frontend Store

`src/stores/host-store.ts` — Zustand store：

- `entries: HostEntry[]`
- `loading: boolean`
- `fetchEntries()` / `createEntry()` / `updateEntry()` / `deleteEntry()` / `toggleEntry()`

### i18n

在 `zh` 和 `en` 的 translation 文件中增加 `hosts` namespace。

## Error Handling

| 场景 | 处理方式 |
|------|----------|
| 用户取消密码输入 | SQLite 已保存成功，Toast 提示"hosts 文件未更新，可稍后在 Hosts 页面点击同步"。不回滚 DB |
| hosts 文件被锁定/只读 | Toast 报错，提示具体原因 |
| hosts 文件格式异常（找不到结束标记） | 重新追加完整的 Meridian 区块，不动原有内容 |
| hostname 验证失败 | 表单字段红色高亮 + Toast 提示 |

**关键决策：** DB 和 hosts 文件不做事务绑定。SQLite 是 source of truth，hosts 文件是"尽力同步"。如果提权失败，数据已保存在 DB 中，用户可以随时通过 `sync_hosts_file` 重新同步。

## Test Strategy

### Backend Unit Tests

| 测试点 | 说明 |
|--------|------|
| CRUD 操作 | 创建/更新/删除/列表/toggle，验证 SQLite 数据正确 |
| hostname 唯一约束 | 重复 hostname 返回 `HOSTNAME_DUPLICATE` |
| IP 格式验证 | 非法 IP 返回 `VALIDATION_ERROR` |
| hostname 格式验证 | 非法主机名返回 `VALIDATION_ERROR` |
| hosts 文件区块生成 | 给定 entries 列表，验证生成的区块内容格式正确 |
| 托管区块解析 | 能正确识别和替换已有的 Meridian 区块，不影响区块外内容 |
| 区块不存在时追加 | hosts 文件无标记时，在末尾正确追加 |
| disabled 条目不写入 | enabled=false 的条目不出现在生成的区块中 |

### Frontend Manual Tests

| 测试点 | 说明 |
|--------|------|
| 添加条目 | 填写表单，验证列表更新 |
| 编辑条目 | 修改 IP/备注，验证保存生效 |
| 删除条目 | 确认弹窗后删除 |
| Toggle 启用/禁用 | 切换后验证 hosts 文件同步 |
| 代理创建后智能提示 | 新建 HTTP 代理，验证弹出 hosts 提示 |
| 代理删除联动 | 删除最后一条使用该域名的代理，验证弹出清理提示 |
| 搜索过滤 | 按关键词过滤列表 |
| 提权取消 | 取消密码输入后，验证 Toast 提示且数据已存 DB |

## Implementation Map

| Spec Item | Code File(s) | Function / Module | Notes |
|-----------|-------------|-------------------|-------|
| Hosts manager 模块 | `src-tauri/src/hosts_manager.rs` | `HostsManager` | 读写系统 hosts 文件、提权、区块管理 |
| DB 操作 | `src-tauri/src/store/` | hosts 相关 SQL | 新增 migration |
| Tauri commands | `src-tauri/src/commands/hosts.rs` | `create_host`, `update_host` 等 | IPC 命令 |
| 输入验证 | `src-tauri/src/validators.rs` | `validate_host_entry` | IP/hostname 格式验证 |
| 前端页面 | `src/pages/HostsPage.tsx` | `HostsPage` | 列表 + 搜索 + 操作 |
| 前端 Store | `src/stores/host-store.ts` | `useHostStore` | Zustand |
| 代理表单集成 | `src/components/proxy/ProxyForm.tsx` | 智能提示 Dialog | 创建/删除时的联动 |
| 侧边栏导航 | `src/components/layout/Sidebar.tsx` | 新增 Hosts 菜单项 | Globe 图标 |
| i18n | `src/i18n/locales/{zh,en}/hosts.json` | | 翻译文件 |
