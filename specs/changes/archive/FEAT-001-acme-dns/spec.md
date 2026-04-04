# Spec: FEAT-001 ACME DNS-01 Certificate Management

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | FEAT-001 Phase 2d |

## Feature Description

通过 ACME 协议 (Let's Encrypt) + DNS-01 challenge 自动申请和续期 SSL/TLS 证书。支持 4 个 DNS 服务商 API 集成。支持通配符域名和多域名 SAN 证书。到期前 30 天自动续期。

## Use Cases

- UC-001: 用户添加 DNS 服务商凭据（Cloudflare / Alidns / DNSPod / Route53）
- UC-002: 测试 DNS 凭据是否可用
- UC-003: 通过 ACME + DNS-01 申请 Let's Encrypt 证书（单域名或多域名 SAN）
- UC-004: 申请通配符证书（`*.example.com`）
- UC-005: 证书到期前 30 天自动续期
- UC-006: 查看续期状态和错误信息
- UC-007: 管理 DNS 凭据（增删改）

## Interface Definition

### Data Models

#### DnsCredential
```typescript
interface DnsCredential {
  id: string;
  name: string;
  provider: "cloudflare" | "alidns" | "dnspod" | "route53";
  credentials_json: string; // masked in list responses
  created_at: string;
  updated_at: string;
}
```

#### Provider Credential Schemas
```typescript
// Cloudflare
{ api_token: string }

// Alidns (Alibaba Cloud)
{ access_key_id: string, access_key_secret: string }

// DNSPod (Tencent Cloud)
{ secret_id: string, secret_key: string }

// Route 53 (AWS)
{ access_key_id: string, secret_access_key: string, region?: string }
```

#### AcmeAccount
```typescript
interface AcmeAccount {
  id: string;
  email: string;
  ca_url: string;
  created_at: string;
}
```

#### RenewalStatus
```typescript
interface RenewalStatus {
  cert_id: string;
  cert_name: string;
  domains: string[];
  expires_at: string;
  auto_renew: boolean;
  last_renew_at: string | null;
  last_renew_error: string | null;
  next_renew_at: string; // expires_at - 30 days
}
```

#### Certificate (MODIFIED — add fields)
```typescript
// Added fields to existing Certificate:
dns_credential_id: string | null;
acme_account_id: string | null;
acme_domains: string | null;      // JSON array: ["example.com", "*.example.com"]
last_renew_error: string | null;
last_renew_at: string | null;
```

### Tauri Commands

#### `list_dns_credentials`
- **Input:** none
- **Response:** `DnsCredential[]`
- **Notes:** `credentials_json` 返回时敏感字段脱敏（只显示最后 4 位）

#### `create_dns_credential`
- **Input:** `{ name: string, provider: string, credentials_json: string }`
- **Response:** `DnsCredential`
- **Validation:** name 非空且唯一，provider 必须为 4 种之一，credentials_json 必须符合对应 provider schema

#### `update_dns_credential`
- **Input:** `{ id: string, name?: string, credentials_json?: string }`
- **Response:** `DnsCredential`

#### `delete_dns_credential`
- **Input:** `{ id: string }`
- **Response:** void
- **Errors:** `CREDENTIAL_IN_USE` — 若有 ACME 证书引用此凭据

#### `test_dns_credential`
- **Input:** `{ id: string }`
- **Response:** `{ success: bool, message: string }`
- **Notes:** 尝试列出 DNS zones/domains 来验证凭据有效。不创建任何记录。
  - Cloudflare: `GET /zones` 
  - Alidns: `DescribeDomains`
  - DNSPod: `DescribeDomainList`
  - Route53: `ListHostedZones`

#### `request_acme_cert`
- **Input:** `{ domains: string[], dns_credential_id: string, email: string, auto_renew: bool }`
- **Response:** `Certificate`
- **Flow:**
  1. 查找或创建 ACME account（按 email 匹配）
  2. 创建 ACME order（所有 domains 作为 identifiers）
  3. 对每个 domain 的 DNS-01 challenge：
     a. 计算 TXT record value
     b. 通过 DNS provider API 创建 `_acme-challenge.{domain}` TXT 记录
     c. 等待 DNS 传播（轮询 verify，最多 120s）
  4. 通知 CA ready → poll order complete
  5. 下载证书链 + 保存文件
  6. 清理 DNS TXT 记录
  7. 写入 DB，reload nginx（如果有使用此域名的规则）
- **Errors:** `DNS_CREDENTIAL_NOT_FOUND`, `DNS_RECORD_FAILED`, `DNS_PROPAGATION_TIMEOUT`, `ACME_ORDER_FAILED`, `ACME_CHALLENGE_FAILED`

#### `get_acme_renewal_status`
- **Input:** none
- **Response:** `RenewalStatus[]` — 所有 `source='acme'` 的证书续期状态

### Internal Functions

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
    /// Create a TXT record. Returns a record identifier for cleanup.
    async fn create_txt_record(&self, fqdn: &str, value: &str) -> Result<String, DnsError>;
    /// Delete a previously created TXT record.
    async fn delete_txt_record(&self, record_id: &str) -> Result<(), DnsError>;
    /// Verify the TXT record is visible (for propagation check).
    async fn verify_propagation(&self, fqdn: &str, value: &str) -> Result<bool, DnsError>;
    /// Test credentials by listing zones. Returns zone count or error.
    async fn test_connection(&self) -> Result<String, DnsError>;
}
```

## Business Rules

1. **仅 DNS-01 challenge** — 不支持 HTTP-01（本地代理不保证公网可达）
2. **仅 Let's Encrypt production** — CA URL 固定 `https://acme-v02.api.letsencrypt.org/directory`
3. **ACME 账户按 email 复用** — 相同 email 不重复注册
4. **DNS 传播超时 120 秒** — 每 5 秒轮询一次，超时则失败
5. **证书有效期 90 天**（Let's Encrypt 默认），30 天前开始续期
6. **续期失败不影响现有证书** — 旧证书继续使用直到真正过期
7. **DNS 凭据删除保护** — 有证书引用时拒绝删除
8. **凭据脱敏** — list 接口返回的 credentials_json 中 secret/token/key 只显示 `****` + 最后 4 位
9. **通配符域名** — `*.example.com` 通过 DNS-01 支持，TXT 记录名为 `_acme-challenge.example.com`
10. **多域名 SAN** — 单次 ACME order 可包含多个 domain，生成一张证书
11. **证书文件命名** — 沿用现有 `cert_{id}.pem` / `cert_{id}.key` 规则
12. **续期后自动 reload nginx** — 如果 nginx 正在运行
13. **DNS TXT 记录清理** — 无论申请成功或失败，最终都要清理 TXT 记录
14. **并发续期限制** — 同一时间只允许一个续期任务运行（Mutex）

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | Create Cloudflare credential | Saved, id returned | |
| TP-002 | Normal | Create Alidns credential | Saved with access_key_id + secret | |
| TP-003 | Normal | List credentials | credentials_json masked (****xxxx) | |
| TP-004 | Normal | Update credential name | Name updated, credentials unchanged | |
| TP-005 | Error | Delete credential in use by cert | CREDENTIAL_IN_USE error | |
| TP-006 | Normal | Delete unused credential | Deleted | |
| TP-007 | Error | Create credential with invalid provider | Validation error | |
| TP-008 | Normal | Test valid Cloudflare credential | success=true, lists zones | |
| TP-009 | Error | Test invalid credential | success=false, error message | |
| TP-010 | Normal | Request ACME cert for single domain | Cert issued, files saved, DB updated | |
| TP-011 | Normal | Request ACME cert for wildcard `*.example.com` | Cert issued with wildcard SAN | |
| TP-012 | Normal | Request ACME cert for multi-domain SAN | Cert with multiple SANs | |
| TP-013 | Error | Request ACME cert with bad DNS credential | DNS_RECORD_FAILED | |
| TP-014 | Error | DNS propagation timeout | DNS_PROPAGATION_TIMEOUT, TXT cleaned up | |
| TP-015 | Normal | Auto-renew cert expiring in 20 days | Cert renewed, files updated, nginx reloaded | |
| TP-016 | Normal | Auto-renew cert expiring in 40 days | No renewal (not in window) | |
| TP-017 | Error | Auto-renew fails | last_renew_error set, old cert still valid | |
| TP-018 | Normal | Get renewal status | Returns list with next_renew_at calculated | |
| TP-019 | Boundary | ACME account reuse — same email, 2nd cert | No new account created | |
| TP-020 | Normal | TXT record cleanup after success | DNS TXT records deleted | |
| TP-021 | Normal | TXT record cleanup after failure | DNS TXT records deleted | |

## UI Spec

### CertsPage — New "DNS Providers" Tab

**Tab layout:** `证书列表 | DNS 服务商`

**DNS Provider list:**
- Table: Name | Provider | Created | Actions (Edit, Test, Delete)
- "Test" button inline, shows success/fail toast
- "Add DNS Provider" button → opens form dialog

**Add/Edit DNS Provider dialog:**
- Name (text input)
- Provider (select: Cloudflare / Alibaba Cloud DNS / DNSPod / AWS Route 53)
- Dynamic credential fields based on provider:
  - Cloudflare: API Token
  - Alidns: AccessKey ID, AccessKey Secret
  - DNSPod: SecretId, SecretKey
  - Route53: Access Key ID, Secret Access Key, Region (optional, default us-east-1)
- All secret fields use `type="password"`
- Save + Test Connection buttons

### CertsPage — ACME Cert Request

**"Request Certificate" button** (alongside existing "Generate Self-Signed" and "Import")

**ACME Request dialog:**
- Domains (textarea, one per line, supports `*.example.com`)
- DNS Provider (select from saved credentials)
- Email (for Let's Encrypt account)
- Auto Renew (toggle, default on)
- Progress indicator during issuance (creating TXT → waiting propagation → verifying → downloading)

### Cert List Enhancements

- Source column: `self-signed` | `imported` | `acme`
- For ACME certs: show renewal info (next renewal date, or last error in red)
- Domains column: show all SAN domains (comma-separated)

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| (filled after Phase 4) | | | |
