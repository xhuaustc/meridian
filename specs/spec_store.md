# Spec: SQLite Store (数据层)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |

## Feature Description

SQLite 数据持久化层，负责所有配置数据的存储、查询、迁移和备份。使用 rusqlite 作为 Rust SQLite 绑定，refinery 管理 schema 迁移。

## Use Cases

- UC-001: 应用启动时自动运行数据库迁移
- UC-002: 代理规则、证书、访问控制列表的增删改查
- UC-003: 应用设置的读写
- UC-004: 数据库备份与还原
- UC-005: 配置导出为 JSON / 从 JSON 导入

## Interface Definition

### Database Initialization

- **Type:** Internal function
- `init_database(data_dir: &Path) -> Result<Database>`
  - 创建或打开 `{data_dir}/meridian.db`
  - 运行 pending migrations
  - 返回连接池

### ProxyRuleRepo

| Function | Input | Output | Notes |
|----------|-------|--------|-------|
| `list(filter: Option<ProxyFilter>)` | 可选筛选条件（type, enabled, keyword） | `Vec<ProxyRule>` | 按 sort_order 排序 |
| `get(id: &str)` | rule UUID | `Option<ProxyRule>` | |
| `create(input: CreateProxyInput)` | 新规则字段 | `ProxyRule` | 自动生成 id + timestamps |
| `update(id: &str, input: UpdateProxyInput)` | 规则 ID + 变更字段 | `ProxyRule` | 更新 updated_at |
| `delete(id: &str)` | rule UUID | `()` | 级联清除 access_list 绑定 |
| `toggle(id: &str, enabled: bool)` | rule UUID + 状态 | `ProxyRule` | |
| `count_by_type()` | — | `HashMap<String, i64>` | 统计各类型规则数量 |

### CertificateRepo

| Function | Input | Output | Notes |
|----------|-------|--------|-------|
| `list()` | — | `Vec<Certificate>` | |
| `get(id: &str)` | cert UUID | `Option<Certificate>` | |
| `create(input: CreateCertInput)` | 证书元数据 | `Certificate` | |
| `delete(id: &str)` | cert UUID | `()` | 检查是否被代理规则引用，有引用则拒绝 |
| `find_expiring(days: i64)` | 天数阈值 | `Vec<Certificate>` | 查找 N 天内到期的证书 |

### AccessListRepo

| Function | Input | Output | Notes |
|----------|-------|--------|-------|
| `list()` | — | `Vec<AccessListWithRules>` | 含关联的 rules |
| `get(id: &str)` | list UUID | `Option<AccessListWithRules>` | |
| `create(input: CreateAccessListInput)` | 名称 + 默认策略 | `AccessList` | |
| `update(id: &str, input: UpdateAccessListInput)` | 变更字段 | `AccessList` | |
| `delete(id: &str)` | list UUID | `()` | 检查是否被代理规则引用 |
| `add_rule(list_id: &str, input: CreateAccessRuleInput)` | IP 规则 | `AccessRule` | |
| `remove_rule(rule_id: &str)` | rule UUID | `()` | |
| `reorder_rules(list_id: &str, rule_ids: Vec<String>)` | 新排序 | `()` | |

### SettingsRepo

| Function | Input | Output | Notes |
|----------|-------|--------|-------|
| `get(key: &str)` | key name | `Option<String>` | |
| `set(key: &str, value: &str)` | key + value | `()` | upsert |
| `get_all()` | — | `HashMap<String, String>` | |

### Backup & Export

| Function | Input | Output | Notes |
|----------|-------|--------|-------|
| `backup(dest: &Path)` | 目标路径 | `()` | SQLite `.backup` API |
| `export_json()` | — | `String` | 所有表数据序列化为 JSON |
| `import_json(data: &str)` | JSON 字符串 | `()` | 清空现有数据后导入，事务保护 |

## Data Model

(See design.md — ProxyRule, Certificate, AccessList, AccessRule, AppSettings tables)

## Business Rules

1. 所有 ID 使用 UUID v4，由 Store 层自动生成
2. `created_at` / `updated_at` 使用 ISO 8601 格式，UTC 时区
3. 删除 Certificate 时，若有 ProxyRule 引用（`certificate_id`），返回错误而非级联删除
4. 删除 AccessList 时，若有 ProxyRule 引用（`access_list_id`），返回错误而非级联删除
5. `import_json` 在单个事务中执行：先清空所有表，再逐表插入。失败时整体回滚
6. Schema migration 前自动备份数据库到 `backups/meridian_{timestamp}.db`
7. `ProxyFilter.keyword` 搜索范围：name, domain, upstream_host（LIKE 模糊匹配）

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | `create` proxy rule with all fields | Rule persisted, ID generated, timestamps set | |
| TP-002 | Normal | `list` with filter `type=http` | Only HTTP rules returned | |
| TP-003 | Normal | `toggle(id, false)` | Rule `enabled=0`, `updated_at` changed | |
| TP-004 | Error | `delete` cert referenced by a proxy rule | Error: "Certificate is in use by rule X" | |
| TP-005 | Error | `delete` access list referenced by a proxy rule | Error: "Access list is in use by rule X" | |
| TP-006 | Normal | `export_json` → `import_json` roundtrip | All data identical after roundtrip | |
| TP-007 | Error | `import_json` with malformed JSON | Error returned, existing data unchanged (rollback) | |
| TP-008 | Normal | `backup` then read backup file | Backup file is valid SQLite DB with same data | |
| TP-009 | Boundary | `create` proxy rule with minimal fields (stream TCP) | domain, path_prefix, certificate_id all NULL | |
| TP-010 | Normal | `find_expiring(30)` with 1 cert expiring in 15 days | Returns that 1 cert | |
| TP-011 | Boundary | `find_expiring(30)` with cert expiring in exactly 30 days | Returns that cert (inclusive) | |
| TP-012 | Normal | `count_by_type` with 2 HTTP + 1 TCP rules | `{"http": 2, "stream_tcp": 1}` | |
| TP-013 | Normal | `list` with keyword filter "api" | Returns rules where name/domain/upstream contains "api" | |
| TP-014 | Combination | Migration + backup: add new column migration | Backup created before migration; migration succeeds; new column available | |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| (filled after Phase 4) | | | |
