# Tasks: FEAT-001 ACME DNS-01 Certificate Management

## Task Table

| ID | Task | Spec Ref | Depends | Est. | Status | AI-Auto |
|----|------|----------|---------|------|--------|---------|
| T-001 | DB migration: add `dns_credentials`, `acme_accounts` tables; alter `certificates` table | spec.md § Data Models | — | 45m | Pending | Yes |
| T-002 | DNS provider trait + Cloudflare implementation | spec.md § DNS Provider trait | T-001 | 60m | Pending | Yes |
| T-003 | Alidns (Alibaba Cloud DNS) provider implementation | spec.md § DNS Provider trait | T-002 | 60m | Pending | Yes |
| T-004 | DNSPod (Tencent Cloud) provider implementation | spec.md § DNS Provider trait | T-002 | 60m | Pending | Yes |
| T-005 | Route 53 (AWS) provider implementation | spec.md § DNS Provider trait | T-002 | 60m | Pending | Yes |
| T-006 | DNS credential CRUD commands + repository | spec.md § Tauri Commands (dns_credential) | T-001 | 45m | Pending | Yes |
| T-007 | DNS credential test command | spec.md § test_dns_credential | T-002,T-006 | 30m | Pending | Yes |
| T-008 | ACME client module (instant-acme integration) | spec.md § request_acme_cert | T-001 | 60m | Pending | Yes |
| T-009 | request_acme_cert command (full flow) | spec.md § request_acme_cert | T-008,T-002,T-006 | 90m | Pending | Yes |
| T-010 | Auto-renewal background task | spec.md § auto_renew_check | T-009 | 60m | Pending | Yes |
| T-011 | get_acme_renewal_status command | spec.md § get_acme_renewal_status | T-001 | 30m | Pending | Yes |
| T-012 | Frontend: DNS Provider management UI (tab + CRUD dialog) | spec.md § UI Spec | T-006,T-007 | 60m | Pending | Yes |
| T-013 | Frontend: ACME cert request dialog + progress | spec.md § UI Spec | T-009,T-012 | 60m | Pending | Yes |
| T-014 | Frontend: cert list enhancements (source, renewal status, SAN domains) | spec.md § UI Spec | T-011 | 45m | Pending | Yes |
| T-015 | i18n: add zh/en translations for all new UI strings | spec.md § UI Spec | T-012,T-013,T-014 | 30m | Pending | Yes |
| T-016 | Cargo.toml: add dependencies (instant-acme, reqwest, hmac, sha2, base64) | design.md § Dependencies | — | 10m | Pending | Yes |

## Dependency Graph

```
T-016 ─┐
       ├─→ T-001 ─→ T-006 ─→ T-007 ─┐
       │      │                        │
       │      ├─→ T-002 ─→ T-003      ├─→ T-012 ─→ T-013 ─→ T-015
       │      │      │    ─→ T-004     │      │
       │      │      │    ─→ T-005     │      │
       │      │      └─────────────────┘      │
       │      │                               │
       │      ├─→ T-008 ─→ T-009 ─→ T-010    │
       │      │                               │
       │      └─→ T-011 ─→ T-014 ────────────┘
       │                                      │
       └──────────────────────────────────────┘
```

## Execution Order (topological)

1. T-016 (deps)
2. T-001 (DB migration)
3. T-002 (trait + Cloudflare) ‖ T-006 (credential CRUD) ‖ T-008 (ACME client) ‖ T-011 (renewal status)
4. T-003 ‖ T-004 ‖ T-005 (other providers, parallel)
5. T-007 (test credential)
6. T-009 (request_acme_cert)
7. T-010 (auto-renewal)
8. T-012 (DNS provider UI)
9. T-013 (ACME request UI) ‖ T-014 (cert list enhancements)
10. T-015 (i18n)
