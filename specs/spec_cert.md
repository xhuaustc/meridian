# Spec: Certificate Manager (证书管理)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |

## Feature Description

SSL/TLS 证书的全生命周期管理：生成自签名证书、导入外部证书、ACME 自动申请（Let's Encrypt）、过期检查与自动续期。

## Use Cases

- UC-001: 生成自签名证书用于本地开发
- UC-002: 上传已有的 cert + key 文件
- UC-003: 通过 ACME 协议申请 Let's Encrypt 证书
- UC-004: 查看证书列表及过期状态
- UC-005: 证书到期前 30 天提醒
- UC-006: ACME 证书自动续期

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

#### `request_acme`
- **Input:** `{ domain: string, email: string, auto_renew: boolean }`
- **Response:** `{ certificate: Certificate }`
- **Errors:** `ACME_CHALLENGE_FAILED`, `DOMAIN_NOT_REACHABLE`
- **Notes:** 使用 HTTP-01 challenge，需要域名可公网解析到本机

#### `list_certs`
- **Input:** none
- **Response:** `{ certificates: Certificate[], expiring_count: number }`

#### `delete_cert`
- **Input:** `{ id: string }`
- **Response:** `{ success: true }`
- **Errors:** `CERT_IN_USE` (被代理规则引用)
- **Side effects:** 删除 cert + key 文件

### Internal Functions

#### `check_expiry() -> Vec<ExpiryWarning>`
- 检查所有证书，返回 30 天内到期的列表
- 应用启动时调用一次

#### `renew_if_needed() -> Result<Vec<RenewalResult>>`
- 遍历 `auto_renew=true` 且 30 天内到期的 ACME 证书
- 自动续期并更新文件 + DB 记录
- 通过 Tauri 定时任务每 24 小时检查一次

## Business Rules

1. **自签名证书**使用 `rcgen` crate 生成，RSA 2048 或 ECDSA P-256
2. **导入证书验证**：解析 PEM 格式，校验 cert 与 key 是否匹配（公钥比对）
3. **文件命名**：`cert_{id}.pem` / `cert_{id}.key`，避免文件名冲突
4. **文件权限**：私钥文件 `0600`，证书文件 `0644`，证书目录 `0700`
5. **删除保护**：若证书被 ProxyRule.certificate_id 引用，拒绝删除并返回引用规则列表
6. **ACME 自动续期窗口**：到期前 30 天开始尝试续期
7. **续期失败处理**：记录错误到 app log，UI 显示续期失败警告，不中断现有证书使用
8. **证书过期不自动禁用代理**：仅在 UI 显示警告，让用户决定处理方式

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

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| (filled after Phase 4) | | | |
