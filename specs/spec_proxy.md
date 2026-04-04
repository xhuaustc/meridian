# Spec: Proxy Management (代理管理)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |
| 2026-04-04 | Add frontend form validation with field error highlighting | UX improvement |
| 2026-04-04 | Add default listen port on proxy type selection (HTTP→80, HTTPS→443) | UX improvement |

## Feature Description

核心功能模块，管理 Layer 4（TCP/UDP Stream）和 Layer 7（HTTP/HTTPS）代理规则。用户通过表单创建规则后，系统自动生成 Nginx 配置并热重载，实现流量转发。

## Use Cases

- UC-001: 添加 HTTP 反向代理规则（域名 + 端口 + 可选路径前缀 → upstream）
- UC-002: 添加 HTTPS 反向代理规则（TLS 终止或透传）
- UC-003: 添加 TCP/UDP Stream 代理规则（端口 → upstream）
- UC-004: 编辑已有规则
- UC-005: 启用/禁用规则（不删除）
- UC-006: 删除规则
- UC-007: 搜索和筛选规则列表
- UC-008: 在 Dashboard 查看统计概览

## Interface Definition

### Tauri Commands

#### `create_proxy`
- **Type:** Tauri IPC command
- **Input:**

| Param | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| name | string | Yes | 1-100 chars | 规则名称 |
| proxy_type | string | Yes | `http` / `stream_tcp` / `stream_udp` | 代理类型 |
| listen_port | number | Yes | 1-65535 | 监听端口 |
| listen_host | string | No | default `0.0.0.0` | 监听地址 |
| domain | string | Conditional | Required if type=http | 域名 |
| path_prefix | string | No | must start with `/` | 路径前缀 |
| upstream_host | string | Yes | IP or hostname | 目标地址 |
| upstream_port | number | Yes | 1-65535 | 目标端口 |
| tls_mode | string | No | `none` / `terminate` / `passthrough`, default `none` | TLS 模式 |
| certificate_id | string | Conditional | Required if tls_mode=terminate | 证书 ID |
| access_list_id | string | No | | 访问控制列表 ID |
| websocket | boolean | No | default false | 启用 WebSocket |
| custom_headers | array | No | `[{op, name, value}]` | 自定义 HTTP 头 |

- **Response (Success):** `{ proxy: ProxyRule }`
- **Error Responses:**

| Code | Condition |
|------|-----------|
| VALIDATION_ERROR | 必填字段缺失或格式错误 |
| PORT_CONFLICT | 端口冲突（见冲突检测规则） |
| CERT_NOT_FOUND | certificate_id 不存在 |
| ACCESS_LIST_NOT_FOUND | access_list_id 不存在 |
| CONFIG_ERROR | Nginx 配置生成或验证失败 |
| ENGINE_ERROR | Nginx reload 失败 |

#### `update_proxy`
- **Input:** Same as create + `id: string` (required)
- **Response:** `{ proxy: ProxyRule }`
- **Errors:** Same as create + `PROXY_NOT_FOUND`

#### `delete_proxy`
- **Input:** `{ id: string }`
- **Response:** `{ success: true }`
- **Side effect:** 重新生成配置并 reload

#### `toggle_proxy`
- **Input:** `{ id: string, enabled: boolean }`
- **Response:** `{ proxy: ProxyRule }`
- **Side effect:** 重新生成配置并 reload

#### `list_proxies`
- **Input:** `{ filter?: { proxy_type?: string, enabled?: boolean, keyword?: string } }`
- **Response:** `{ proxies: ProxyRule[], stats: { total, active, by_type } }`

#### `check_port_conflict`
- **Input:** `{ listen_port: number, proxy_type: string, domain?: string, path_prefix?: string, exclude_id?: string }`
- **Response:** `{ conflict: boolean, conflict_type?: string, conflicting_rules?: ProxyRule[] }`
- **Notes:** `exclude_id` 用于编辑时排除自身

## Data Model

(See design.md — ProxyRule table)

## Business Rules

1. **HTTP/HTTPS 规则必须指定域名**（domain 字段），Stream 规则不需要
2. **TLS terminate 模式必须绑定证书**（certificate_id）
3. **TLS passthrough 模式不绑定证书**（Nginx 不解密，直接转发）
4. **path_prefix 必须以 `/` 开头**，如 `/api`、`/v1`
5. **端口冲突规则**（详见 design.md Config Engine 部分）：
   - 同端口 + 不同域名 → 允许（虚拟主机）
   - 同端口 + 同域名 + 不同 path_prefix → 允许（location 路由）
   - 同端口 + 同域名 + 同 path_prefix → 冲突
   - Stream 同端口 → 冲突（端口独占）
   - HTTP/HTTPS 与 Stream 同端口 → 冲突
6. **规则变更流程**：Save to DB → Generate config → nginx -t → reload。任一步骤失败则回滚
7. **禁用规则**：不删除 DB 记录，仅在生成配置时跳过该规则
8. **WebSocket 仅对 HTTP/HTTPS 类型有效**
9. **Custom headers 仅对 HTTP/HTTPS 类型有效**
10. **前端表单验证**：保存前检查所有必填字段（name, listen_port, upstream_host, upstream_port, domain for HTTP/HTTPS），不合格字段添加红色边框高亮，第一个错误以 toast 提示。同时执行异步端口冲突检查。
11. **默认监听端口**：创建新规则时（非编辑），选择 HTTP 类型自动填入 80，选择 HTTPS 自动填入 443，TCP/UDP 留空

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | Create HTTP rule: `app.local:80/ → 127.0.0.1:3000` | Rule created, nginx reloaded, proxy works | |
| TP-002 | Normal | Create HTTPS rule with TLS terminate + cert | Rule created, HTTPS proxy works | |
| TP-003 | Normal | Create TCP stream: `:15432 → 192.168.1.50:5432` | Rule created, TCP forwarding works | |
| TP-004 | Normal | Create two HTTP rules on port 80 with different domains | Both rules work, virtual host routing correct | Multi-domain same port |
| TP-005 | Normal | Create two rules on same domain, different path_prefix | Both rules work, location routing correct | |
| TP-006 | Error | Create HTTP rule without domain | VALIDATION_ERROR | |
| TP-007 | Error | Create TLS terminate rule without certificate_id | VALIDATION_ERROR | |
| TP-008 | Error | Create TCP stream on port already used by another TCP stream | PORT_CONFLICT with conflicting rule info | |
| TP-009 | Error | Create HTTP rule on port used by TCP stream | PORT_CONFLICT (http vs stream) | |
| TP-010 | Error | Create rule with same domain + same port + same path_prefix | PORT_CONFLICT | |
| TP-011 | Normal | Toggle rule off → verify config regenerated | Rule disabled, traffic no longer proxied | |
| TP-012 | Normal | Toggle rule on → verify config regenerated | Rule re-enabled, traffic resumes | |
| TP-013 | Normal | Delete rule → verify config regenerated | Rule removed from DB and nginx config | |
| TP-014 | Normal | List with filter `proxy_type=http` | Only HTTP/HTTPS rules returned | |
| TP-015 | Normal | List with keyword filter "api" | Rules matching name/domain/upstream | |
| TP-016 | Error | Update rule to create port conflict | PORT_CONFLICT, original rule unchanged | |
| TP-017 | Boundary | Create rule with port 1 (privileged) | Rule created (may need root/admin to actually bind) | |
| TP-018 | Boundary | Create rule with port 65535 | Rule created successfully | |
| TP-019 | Normal | check_port_conflict while editing rule (exclude_id=self) | No conflict with itself | |
| TP-020 | Combination | Create HTTPS terminate + access list + WebSocket + custom headers | All features active simultaneously | |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Proxy CRUD commands | `src-tauri/src/commands/proxy.rs` | `create_proxy`, `update_proxy`, `delete_proxy`, `toggle_proxy`, `list_proxies` | |
| Port conflict check | `src-tauri/src/commands/engine.rs` | `check_port_conflict` | |
| Input validation | `src-tauri/src/validators.rs` | `validate_create_proxy`, `validate_update_proxy` | |
| Frontend proxy form | `src/components/proxy/ProxyForm.tsx` | `ProxyForm` | Field validation + default ports |
| Frontend proxy store | `src/stores/proxy-store.ts` | `useProxyStore` | Zustand store |
