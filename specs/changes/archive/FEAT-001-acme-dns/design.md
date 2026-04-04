# Design: FEAT-001 ACME DNS-01 Certificate Management

## Overview

Add ACME protocol support using DNS-01 challenge for automated certificate issuance and renewal. Supports 4 DNS providers, wildcard domains, and multi-domain SAN certificates. Only free CA providers (Let's Encrypt).

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Cert Manager (existing)             │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │
│  │Self-sign │  │ Import   │  │ ACME Manager  │  │
│  │(rcgen)   │  │(PEM)     │  │   (NEW)       │  │
│  └──────────┘  └──────────┘  └───────┬───────┘  │
│                                      │           │
│                          ┌───────────▼────────┐  │
│                          │  DNS Provider      │  │
│                          │  Abstraction Layer  │  │
│                          │  (trait DnsProvider)│  │
│                          └──┬──┬──┬──┬───────┘  │
│                             │  │  │  │           │
│              ┌──────────────┘  │  │  └────────┐  │
│              ▼        ▼        ▼        ▼     │  │
│         Cloudflare  Alidns  DNSPod  Route53   │  │
│                                               │  │
│                          ┌────────────────┐   │  │
│                          │ DNS Credentials │   │  │
│                          │ (SQLite table)  │   │  │
│                          └────────────────┘   │  │
└───────────────────────────────────────────────┘
```

## Module Breakdown

### 1. DNS Provider Abstraction (`dns_provider/`)

**Trait:**
```rust
#[async_trait]
trait DnsProvider: Send + Sync {
    async fn create_txt_record(domain: &str, value: &str) -> Result<String>;  // returns record_id
    async fn delete_txt_record(record_id: &str) -> Result<()>;
    async fn verify_txt_record(domain: &str, value: &str) -> Result<bool>;
}
```

**Providers:**
| Provider | Auth | API |
|----------|------|-----|
| Cloudflare | API Token (Zone:DNS:Edit) | REST v4 `zones/{id}/dns_records` |
| Alibaba Cloud DNS | AccessKey ID + Secret | OpenAPI `alidns.cn-hangzhou.aliyuncs.com` |
| DNSPod | SecretId + SecretKey | REST v3 `dnspod.tencentcloudapi.com` |
| AWS Route 53 | Access Key + Secret Key | AWS SDK `route53.ChangeResourceRecordSets` |

### 2. ACME Client (`acme/`)

- Uses `instant-acme` crate (pure Rust, async, no OpenSSL dependency)
- Flow: create account → new order → DNS-01 challenge → set TXT record → notify ready → poll → download cert
- Supports multi-domain SAN (one order with multiple identifiers)
- Supports wildcard (`*.example.com`)
- CA: Let's Encrypt production only (`https://acme-v02.api.letsencrypt.org/directory`)
- Account key stored in SQLite (PEM format)

### 3. Data Model Changes

**New table: `dns_credentials`**
| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| name | TEXT | User-friendly name |
| provider | TEXT | "cloudflare" / "alidns" / "dnspod" / "route53" |
| credentials_json | TEXT | Provider-specific JSON blob |
| created_at | TEXT | ISO 8601 |
| updated_at | TEXT | ISO 8601 |

**credentials_json format per provider:**
- Cloudflare: `{"api_token": "..."}`
- Alidns: `{"access_key_id": "...", "access_key_secret": "..."}`
- DNSPod: `{"secret_id": "...", "secret_key": "..."}`
- Route53: `{"access_key_id": "...", "secret_access_key": "...", "region": "us-east-1"}`

**New table: `acme_accounts`**
| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | UUID |
| email | TEXT | ACME registration email |
| account_key_pem | TEXT | ECDSA P-256 private key |
| ca_url | TEXT | `https://acme-v02.api.letsencrypt.org/directory` |
| created_at | TEXT | ISO 8601 |

**Modified table: `certificates` — add columns:**
| Column | Type | Notes |
|--------|------|-------|
| dns_credential_id | TEXT NULL | FK to dns_credentials |
| acme_account_id | TEXT NULL | FK to acme_accounts |
| acme_domains | TEXT NULL | JSON array of domains for SAN cert |
| last_renew_error | TEXT NULL | Last renewal failure message |
| last_renew_at | TEXT NULL | ISO 8601 of last renewal attempt |

### 4. Auto-Renewal

- On app startup: check all `source='acme' AND auto_renew=true` certs
- Every 12 hours: re-check
- Renew if expires_at - now < 30 days
- On success: update cert files + DB record + reload nginx
- On failure: set `last_renew_error`, log to error.log, continue

### 5. New Tauri Commands

| Command | Input | Output |
|---------|-------|--------|
| `list_dns_credentials` | — | `DnsCredential[]` |
| `create_dns_credential` | `{name, provider, credentials}` | `DnsCredential` |
| `update_dns_credential` | `{id, name?, credentials?}` | `DnsCredential` |
| `delete_dns_credential` | `{id}` | `void` |
| `test_dns_credential` | `{id}` | `{success, message}` |
| `request_acme_cert` | `{domains[], dns_credential_id, email, auto_renew}` | `Certificate` |
| `get_acme_renewal_status` | — | `RenewalStatus[]` |

### 6. UI Changes (CertsPage)

- Add "DNS Providers" tab alongside certificate list
- DNS Provider CRUD: name, provider type select, credential fields (dynamic per provider)
- "Test Connection" button per provider
- ACME cert request dialog: domain input (multi-line for SAN), DNS provider select, email, auto-renew toggle
- Renewal status column in cert list (next renewal date, last error)

## Key Dependencies

**Rust crates (new):**
- `instant-acme` — ACME protocol client
- `reqwest` — HTTP client for DNS provider APIs (already via tauri)
- `hmac` + `sha2` — for Alidns/DNSPod request signing
- `base64` — encoding

**No new frontend dependencies needed.**

## ADR: Why DNS-01 only (no HTTP-01)

DNS-01 is the only ACME challenge type that supports wildcard certificates and works regardless of whether the machine is publicly accessible. Since this is a local proxy manager, HTTP-01 would require port 80 to be reachable from the internet, which is unlikely in most use cases. DNS-01 via provider APIs is universally applicable.
