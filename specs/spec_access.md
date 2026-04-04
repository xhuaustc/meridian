# Spec: Access Control (访问控制)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |

## Feature Description

基于 IP 的访问控制列表（Access List）管理。用户创建命名的规则集，添加 allow/deny 规则（支持单 IP 和 CIDR），然后将规则集绑定到代理规则。Config Engine 将其转换为 Nginx 的 `allow` / `deny` 指令。

## Use Cases

- UC-001: 创建 Access List（如 "Office Network Only"）
- UC-002: 添加/删除/排序 IP 规则
- UC-003: 将 Access List 绑定到代理规则
- UC-004: 查看哪些代理规则引用了某个 Access List

## Interface Definition

### Tauri Commands

#### `list_access_lists`
- **Input:** none
- **Response:** `{ access_lists: AccessListWithRules[] }`
- **AccessListWithRules:** `{ ...AccessList, rules: AccessRule[], bound_proxies: ProxyRule[] }`

#### `create_access_list`
- **Input:** `{ name: string, default_policy: "allow" | "deny" }`
- **Response:** `{ access_list: AccessList }`
- **Errors:** `NAME_DUPLICATE`

#### `update_access_list`
- **Input:** `{ id: string, name?: string, default_policy?: "allow" | "deny" }`
- **Response:** `{ access_list: AccessList }`
- **Side effects:** 若有绑定的代理规则，触发 nginx 配置重新生成 + reload

#### `delete_access_list`
- **Input:** `{ id: string }`
- **Errors:** `ACCESS_LIST_IN_USE` (有代理规则引用)
- **Response:** `{ success: true }`

#### `add_access_rule`
- **Input:** `{ access_list_id: string, action: "allow" | "deny", ip_cidr: string }`
- **Response:** `{ rule: AccessRule }`
- **Errors:** `INVALID_IP_CIDR`, `DUPLICATE_RULE`
- **Side effects:** 若 access list 被绑定，触发 nginx reload

#### `remove_access_rule`
- **Input:** `{ rule_id: string }`
- **Response:** `{ success: true }`
- **Side effects:** 若 access list 被绑定，触发 nginx reload

#### `reorder_access_rules`
- **Input:** `{ access_list_id: string, rule_ids: string[] }`
- **Response:** `{ success: true }`
- **Notes:** rule_ids 顺序决定匹配顺序

## Business Rules

1. **Access List 名称唯一**（大小写不敏感）
2. **IP/CIDR 格式验证**：必须是有效的 IPv4/IPv6 地址或 CIDR 表示法
3. **规则按 sort_order 顺序匹配**，Nginx 按上到下执行 allow/deny
4. **默认策略**附加在所有规则之后：`default_policy=deny` → 末尾添加 `deny all`
5. **删除保护**：被代理规则引用的 Access List 不可删除
6. **级联更新**：修改 Access List（增删规则、改默认策略）后，所有绑定的代理规则需重新生成 Nginx 配置并 reload
7. **同一 Access List 内不允许重复 IP/CIDR + action 组合**

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | Create access list "Office" with default deny | List created | |
| TP-002 | Error | Create access list with duplicate name | NAME_DUPLICATE error | |
| TP-003 | Normal | Add rule: allow 192.168.1.0/24 | Rule added, sort_order assigned | |
| TP-004 | Normal | Add rule: deny 10.0.0.5 (single IP) | Rule added | |
| TP-005 | Error | Add rule: invalid IP "abc.def" | INVALID_IP_CIDR error | |
| TP-006 | Error | Add rule: duplicate (same list + action + ip_cidr) | DUPLICATE_RULE error | |
| TP-007 | Normal | Reorder rules: [rule_c, rule_a, rule_b] | sort_order updated to reflect new order | |
| TP-008 | Normal | Delete access list (not referenced) | List + rules deleted | |
| TP-009 | Error | Delete access list (referenced by proxy) | ACCESS_LIST_IN_USE with proxy names | |
| TP-010 | Normal | Update access list → verify nginx reload | Config regenerated with new rules, nginx reloaded | |
| TP-011 | Boundary | Access list with 0 rules + default deny | Nginx config: just `deny all;` | |
| TP-012 | Normal | Access list bound to 2 proxy rules → add IP rule | Both proxy configs updated | |
| TP-013 | Combination | Create list → add rules → bind to proxy → modify list → verify | End-to-end: list, rules, binding, nginx config all correct | |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| (filled after Phase 4) | | | |
