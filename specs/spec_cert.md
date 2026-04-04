# Spec: Certificate Manager (证书管理)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |
| 2026-04-04 | Merge ACME DNS-01 certificate management (FEAT-001) | Feature merge |
| 2026-04-04 | Add certificate export feature (zip: cert.pem + key.pem) | Enhancement |

## Feature Description

SSL/TLS 证书的全生命周期管理：生成自签名证书、导入外部证书、通过 ACME 协议 (Let's Encrypt) + DNS-01 challenge 自动申请和续期证书。支持 4 个 DNS 服务商 API 集成（Cloudflare / Alidns / DNSPod / Route53），支持通配符域名和多域名 SAN 证书。

## Use Cases

- UC-001: 生成自签名证书用于本地开发
- UC-002: 上传已有的 cert + key 文件
- UC-003: 添加 DNS 服务商凭据（Cloudflare / Alidns / DNSPod / Route53）
- UC-004: 测试 DNS 凭据是否可用
- UC-005: 通过 ACME + DNS-01 申请 Let's Encrypt 证书（单域名、多域名 SAN、通配符）
- UC-006: 查看证书列表及过期状态
- UC-007: 证书到期前 30 天自动续期
- UC-008: 查看续期状态和错误信息
- UC-009: 管理 DNS 凭据（增删改）
- UC-010: 导出证书文件（cert.pem + key.pem 打包为 zip）用于其他系统部署或备份

## Interface Definition

### Tauri Commands

#### `generate_self_signed`
- **Input:** `{ domain: string, validity_days?: number }`
- **Default validity_days:** 365
- **Response:** `{ certificate: Certificate }`
- **Side effects:** 生成 cert.pem + key.pem 写入 `data/nginx/certs/`，权限设为 0600

#### `import_cert`
- **Input:** `{ name: string, domain: string, cert_content: string, key_content: string }`
- **Response:** `{ certificate: Certificate }`
- **Errors:** `INVALID_CERT` (格式错误/cert-key 不匹配), `DOMAIN_MISMATCH`
- **Side effects:** 写入文件，权限设为 0600

#### `list_dns_credentials`
- **Input:** none
- **Response:** `DnsCredential[]`
- **Notes:** `credentials_json` 返回时敏感字段脱敏（只显示最后 4 位）

#### `create_dns_credential`
- **Input:** `{ name: string, provider: string, credentials_json: string }`
- **Response:** `DnsCredential`
- **Validation:** name 非空且唯一，provider 必须为 4 种之一

#### `update_dns_credential`
- **Input:** `{ id: string, name?: string, credentials_json?: string }`
- **Response:** `DnsCredential`

#### `delete_dns_credential`
- **Input:** `{ id: string }`
- **Errors:** `CREDENTIAL_IN_USE` — 若有 ACME 证书引用此凭据

#### `test_dns_credential`
- **Input:** `{ id: string }`
- **Response:** `{ success: bool, message: string }`
- **Notes:** 尝试列出 DNS zones/domains 来验证凭据有效

#### `request_acme_cert`
- **Input:** `{ domains: string[], dns_credential_id: string, email: string, auto_renew: bool }`
- **Response:** `Certificate`
- **Flow:** Create/reuse ACME account → create order → DNS-01 challenge (create TXT, wait propagation ≤120s) → download cert → cleanup TXT → save files + DB → reload nginx
- **Errors:** `DNS_CREDENTIAL_NOT_FOUND`, `DNS_RECORD_FAILED`, `DNS_PROPAGATION_TIMEOUT`, `ACME_ORDER_FAILED`, `ACME_CHALLENGE_FAILED`

#### `get_acme_renewal_status`
- **Input:** none
- **Response:** `RenewalStatus[]` — 所有 `source='acme'` 的证书续期状态

#### `list_certs`
- **Input:** none
- **Response:** `{ certificates: Certificate[], expiring_count: number }`

#### `delete_cert`
- **Input:** `{ id: string }`
- **Response:** `{ success: true }`
- **Errors:** `CERT_IN_USE` (被代理规则引用)
- **Side effects:** 删除 cert + key 文件

#### `export_certificate`
- **Input:** `{ id: string }`
- **Response:** `{ cert_pem: string, key_pem: string, domain: string, name: string }`
- **Errors:** `CERT_NOT_FOUND`, `FILE_READ_ERROR` (cert/key 文件不存在或不可读)
- **Notes:** 读取 cert_path 和 key_path 的文件内容，以 PEM 字符串返回。前端负责打包为 zip 并通过系统保存对话框写入磁盘。仅 `status=ready` 的证书可导出。

### Internal Functions

#### `check_expiry() -> Vec<ExpiryWarning>`
- 检查所有证书，返回 30 天内到期的列表
- 应用启动时调用一次

#### `auto_renew_check()`
- 启动时调用 + 每 12 小时定时调用
- 筛选 `source='acme' AND auto_renew=true AND expires_at - now < 30d`
- 对每个到期证书执行续期（复用 `request_acme_cert` 逻辑）
- 成功：更新文件 + DB + reload nginx
- 失败：记录 `last_renew_error` + 写入 error.log

#### DNS Provider trait
```rust
#[async_trait]
pub trait DnsProvider: Send + Sync {
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, DnsError>;
    async fn delete_txt_record(&self, record_id: &str) -> Result<(), DnsError>;
    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, DnsError>;
    async fn test_connection(&self) -> Result<String, DnsError>;
}
```

### Data Models (ACME additions)

#### DnsCredential
```typescript
interface DnsCredential {
  id: string; name: string;
  provider: "cloudflare" | "alidns" | "dnspod" | "route53";
  credentials_json: string; // masked in list responses
  created_at: string; updated_at: string;
}
```

#### Certificate (added fields for ACME)
```typescript
dns_credential_id: string | null;
acme_account_id: string | null;
acme_domains: string | null;      // JSON array
last_renew_error: string | null;
last_renew_at: string | null;
```

#### RenewalStatus
```typescript
interface RenewalStatus {
  cert_id: string; cert_name: string; domains: string[];
  expires_at: string; auto_renew: boolean;
  last_renew_at: string | null; last_renew_error: string | null;
  next_renew_at: string; // expires_at - 30 days
}
```

## Business Rules

1. **自签名证书**使用 `rcgen` crate 生成，RSA 2048 或 ECDSA P-256
2. **导入证书验证**：解析 PEM 格式，校验 cert 与 key 是否匹配（公钥比对）
3. **文件命名**：`cert_{id}.pem` / `cert_{id}.key`，避免文件名冲突
4. **文件权限**：私钥文件 `0600`，证书文件 `0644`，证书目录 `0700`
5. **删除保护**：若证书被 ProxyRule.certificate_id 引用，拒绝删除并返回引用规则列表
6. **ACME 自动续期窗口**：到期前 30 天开始尝试续期
7. **续期失败处理**：记录错误到 app log，UI 显示续期失败警告，不中断现有证书使用
8. **证书过期不自动禁用代理**：仅在 UI 显示警告，让用户决定处理方式
9. **仅 DNS-01 challenge** — 不支持 HTTP-01（本地代理不保证公网可达）
10. **仅 Let's Encrypt production** — CA URL 固定 `https://acme-v02.api.letsencrypt.org/directory`
11. **ACME 账户按 email 复用** — 相同 email 不重复注册
12. **DNS 传播超时 120 秒** — 每 5 秒轮询一次
13. **证书有效期 90 天**（Let's Encrypt 默认），30 天前开始续期
14. **续期失败不影响现有证书** — 旧证书继续使用直到真正过期
15. **DNS 凭据删除保护** — 有证书引用时拒绝删除
16. **凭据脱敏** — list 接口返回的 credentials_json 中 secret/token/key 只显示 `****` + 最后 4 位
17. **通配符域名** — `*.example.com` 通过 DNS-01 支持
18. **多域名 SAN** — 单次 ACME order 可包含多个 domain
19. **DNS TXT 记录清理** — 无论申请成功或失败，最终都要清理 TXT 记录
20. **并发续期限制** — 同一时间只允许一个续期任务运行（Mutex）
21. **证书导出**：将 cert.pem + key.pem 打包为 zip 文件导出，文件命名 `{domain}.cert.pem` / `{domain}.key.pem`，zip 命名 `{domain}_{name}.zip`。仅 `status=ready` 的证书可导出（pending/failed 禁用导出按钮）。通配符域名 `*` 替换为 `_wildcard` 以兼容文件系统。

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | generate_self_signed("app.local") | Cert created, files exist, key permission 0600 | |
| TP-002 | Normal | generate_self_signed("app.local", 30) | expires_at = now + 30 days | |
| TP-003 | Normal | import valid cert + key pair | Cert imported, files copied | |
| TP-004 | Error | import cert with mismatched key | INVALID_CERT error | |
| TP-005 | Error | import malformed PEM content | INVALID_CERT error | |
| TP-006 | Normal | list_certs with 3 certs (1 expiring) | Returns 3 certs, expiring_count=1 | |
| TP-007 | Normal | delete_cert not referenced by any rule | Cert deleted, files removed | |
| TP-008 | Error | delete_cert referenced by a proxy rule | CERT_IN_USE with rule names | |
| TP-009 | Normal | check_expiry with cert expiring in 15 days | Warning returned for that cert | |
| TP-010 | Boundary | check_expiry with cert expiring in exactly 30 days | Warning returned (inclusive) | |
| TP-011 | Boundary | check_expiry with cert expiring in 31 days | No warning | |
| TP-012 | Normal | Verify key file permissions after generate | Unix mode 0600 | Linux/macOS |
| TP-013 | Normal | Verify cert directory permissions | Unix mode 0700 | Linux/macOS |
| TP-014 | Combination | Generate self-signed → bind to proxy rule → delete cert | Delete blocked with CERT_IN_USE | |
| TP-015 | Normal | Create Cloudflare DNS credential | Saved, id returned | ACME |
| TP-016 | Normal | List DNS credentials | credentials_json masked (****xxxx) | ACME |
| TP-017 | Error | Delete DNS credential in use by cert | CREDENTIAL_IN_USE error | ACME |
| TP-018 | Normal | Test valid DNS credential | success=true | ACME |
| TP-019 | Error | Test invalid DNS credential | success=false, error message | ACME |
| TP-020 | Normal | Request ACME cert for single domain | Cert issued, files saved | ACME |
| TP-021 | Normal | Request ACME cert for wildcard `*.example.com` | Cert issued with wildcard SAN | ACME |
| TP-022 | Normal | Request ACME cert for multi-domain SAN | Cert with multiple SANs | ACME |
| TP-023 | Error | ACME request with bad DNS credential | DNS_RECORD_FAILED | ACME |
| TP-024 | Normal | Auto-renew cert expiring in 20 days | Cert renewed, files updated, nginx reloaded | ACME |
| TP-025 | Error | Auto-renew fails | last_renew_error set, old cert still valid | ACME |
| TP-026 | Normal | Get renewal status | Returns list with next_renew_at | ACME |
| TP-027 | Normal | TXT record cleanup after success/failure | DNS TXT records deleted | ACME |
| TP-028 | Normal | Export ready cert → save dialog → zip saved | Zip contains {domain}.cert.pem + {domain}.key.pem | Export |
| TP-029 | Normal | Export wildcard cert `*.example.com` | Zip named `_wildcard.example.com_xxx.zip` | Export |
| TP-030 | Error | Export cert with missing file on disk | FILE_READ_ERROR | Export |
| TP-031 | Boundary | Export button on pending cert | Button disabled | Export |
| TP-032 | Boundary | Export button on failed cert | Button disabled | Export |

## UI Additions (ACME)

### CertsPage — DNS Providers Tab
- Table: Name | Provider | Created | Actions (Edit, Test, Delete)
- Add/Edit dialog with provider-specific credential fields (all secret fields `type="password"`)

### CertsPage — ACME Cert Request
- "Request Certificate" button alongside existing actions
- Dialog: Domains (textarea), DNS Provider (select), Email, Auto Renew (toggle)
- Progress indicator during issuance

### Cert List Enhancements
- Source column: `self-signed` | `imported` | `acme`
- ACME certs: renewal info (next date, or last error in red)

### Certificate Export
- **位置：** 每张证书卡片的操作区域，在删除按钮左侧添加 Download 图标按钮（`lucide-react` Download icon）
- **交互：**
  1. 点击导出按钮 → 前端弹出 Tauri save dialog（`@tauri-apps/plugin-dialog`），默认文件名 `{domain}_{name}.zip`
  2. 用户选择保存路径后，调用 `export_certificate(id, save_path)`
  3. 后端读取 cert/key 文件，使用 `zip` crate 打包写入指定路径
  4. 成功 → toast 提示；失败 → error toast
- **禁用条件：** `status !== 'ready'`（pending/failed 状态下按钮灰显不可点击）
- **文件内容：** zip 内含 `{domain}.cert.pem` 和 `{domain}.key.pem`
- **i18n keys：** `certs.export`（按钮 tooltip）, `certs.exportSuccess`, `certs.exportError`

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Cert CRUD commands | `src-tauri/src/commands/cert.rs` | `generate_self_signed_cert`, `import_certificate`, `delete_certificate`, `list_certificates` | |
| Cert manager | `src-tauri/src/cert_manager.rs` | `generate_self_signed()`, `import_cert()` | |
| DNS credential commands | `src-tauri/src/commands/dns_credential.rs` | `list_dns_credentials`, `create_dns_credential`, etc. | |
| DNS provider trait | `src-tauri/src/dns_provider/mod.rs` | `DnsProvider` trait | |
| Cloudflare provider | `src-tauri/src/dns_provider/cloudflare.rs` | `CloudflareProvider` | |
| Alidns provider | `src-tauri/src/dns_provider/alidns.rs` | `AlidnsProvider` | |
| DNSPod provider | `src-tauri/src/dns_provider/dnspod.rs` | `DnspodProvider` | |
| Route53 provider | `src-tauri/src/dns_provider/route53.rs` | `Route53Provider` | |
| ACME client | `src-tauri/src/acme_client/mod.rs` | `request_certificate()` | |
| Auto-renewal | `src-tauri/src/acme_client/renewal.rs` | `spawn_renewal_task()` | 12h interval |
| ACME commands | `src-tauri/src/commands/acme.rs` | `request_acme_cert`, `get_acme_renewal_status` | |
| Export certificate | `src-tauri/src/commands/cert.rs` | `export_certificate` | zip crate, reads cert/key files |
| Export UI | `src/pages/CertsPage.tsx` | `handleExport()` | Download button + save dialog |
