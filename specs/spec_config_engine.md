# Spec: Config Engine (配置引擎)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |
| 2026-04-04 | Fix: quote all file paths in generated nginx configs for dirs with spaces | Bug fix |
| 2026-04-04 | Add custom JSON log_format (meridian, stream_meridian) for monitoring metrics | FEAT-002 merge |
| 2026-04-04 | Add per-rule access_log directives for metrics collection | FEAT-002 merge |
| 2026-04-04 | Add data_dir parameter to config generation functions | FEAT-002 merge |
| 2026-04-05 | Add custom 502 error page (error_pages module) | UX improvement |
| 2026-04-05 | Add configurable worker_processes setting (default "2", supports "auto") | Performance tuning |
| 2026-04-05 | Generate nginx.conf on every app startup, not just auto-start | Bug fix: first-launch |
| 2026-11-17 | Synthesise a port-80 308 redirect block for rules with `force_https` | Ticket #22 |
| 2026-11-17 | Default security response headers (3, `always`) with per-hostname switches | Ticket #20 |
| 2026-11-17 | Emit `Strict-Transport-Security` on TLS-terminating server blocks only, behind a per-hostname switch | Ticket #23 |
| 2026-11-17 | Add the settings-page overview of stored hostname settings: rule reference counts, orphan flag and orphan HSTS count | Ticket #25 |

## Feature Description

将 SQLite 中的代理规则转换为 Nginx 配置文件。负责配置生成、端口冲突检测、配置文件写入与回滚。配置文件按端口聚合（HTTP/HTTPS），按规则独立（Stream）。

## Use Cases

- UC-001: 从 DB 规则生成完整 Nginx 配置文件集
- UC-002: 检测端口冲突
- UC-003: 配置变更失败时回滚到上一版

## Interface Definition

### `generate_all_configs`
- **Type:** Internal function
- **Input:** `(data_dir: &Path, rules: &[ProxyRule], certs: &[Certificate], access_lists: &[AccessListWithRules])`
- **Output:** `Result<ConfigBundle>` — `ConfigBundle { main_conf: String, http_confs: Vec<(String, String)>, stream_confs: Vec<(String, String)> }`
- **Notes:** `http_confs` 的 key 是文件名（如 `port_80.conf`），value 是配置内容。`data_dir` 用于生成日志文件路径。Worker processes 使用默认值 "2"。

### `generate_all_configs_with_settings`
- **Type:** Internal function
- **Input:** `(data_dir, rules, certs, access_lists, hostname_settings: &HashMap<String, HostnameSettings>, worker_processes: &str)`
- **Output:** 同 `generate_all_configs`
- **Notes:** 支持传入 `worker_processes` 设置值（"auto" 或数字字符串）和按 hostname 索引的安全设置。由 `apply_db_state`、配置预览和应用启动流程调用，两处设置都从数据库读取。

### `validate_port_conflicts`
- **Type:** Internal function
- **Input:** `(rules: &[ProxyRule], exclude_id: Option<&str>)`
- **Output:** `Result<Vec<PortConflict>>`
- **PortConflict:** `{ rule_a_id, rule_b_id, port, conflict_type: "stream_port_exclusive" | "http_stream_clash" | "duplicate_route" }`

### `write_configs`
- **Type:** Internal function
- **Input:** `(bundle: &ConfigBundle, output_dir: &Path)`
- **Output:** `Result<()>`
- **Side effects:**
  1. 备份当前 conf.d/ 和 stream.d/ 到临时目录
  2. 清空 conf.d/ 和 stream.d/
  3. 写入新配置文件
  4. 若后续 nginx -t 失败，调用 `restore_previous_configs` 还原

### `restore_previous_configs`
- **Type:** Internal function
- **Input:** `(backup_dir: &Path, output_dir: &Path)`
- **Output:** `Result<()>`

## Business Rules

1. **HTTP/HTTPS 按端口聚合**：同一监听端口的所有 HTTP/HTTPS 规则聚合到一个配置文件 `conf.d/port_{N}.conf`，每个域名一个 `server` block
2. **同域名多路径**：同域名 + 同端口的不同 path_prefix 规则合并到同一个 `server` block 的多个 `location` 块
3. **Stream 规则独立文件**：每条 Stream 规则生成独立配置 `stream.d/stream_{id}.conf`
4. **禁用规则不生成配置**：`enabled=0` 的规则跳过
5. **主配置模板**：`nginx.conf` 包含 `worker_processes {N};`（从设置读取，默认 "2"，支持 "auto"）+ `events {}` + `http { log_format meridian ...; include conf.d/*.conf; }` + `stream { log_format stream_meridian ...; include stream.d/*.conf; }`
5a. **HTTP JSON 日志格式 (`meridian`)**：在 `http` block 中定义 `log_format meridian escape=json`，字段包括 `time`, `remote_addr`, `method`, `uri`, `status`, `body_bytes_sent`, `request_time`, `upstream_response_time`, `host`
5b. **Stream JSON 日志格式 (`stream_meridian`)**：在 `stream` block 中定义 `log_format stream_meridian`，字段包括 `time`, `remote_addr`, `protocol`, `status`, `bytes_sent`, `bytes_received`, `session_time`
5c. **Per-rule access log**：每个 HTTP `location` block 和 Stream `server` block 添加 `access_log "{data_dir}/nginx/logs/rule_{id}.access.log" meridian;`（HTTP 用 meridian 格式，Stream 用 stream_meridian 格式）
5d. **全局日志继承保持**：HTTP `location` block 中设置 `access_log` 会覆盖 `http` 级别继承，因此每个 location 需同时写入 per-rule 日志和全局 `access.log`（两条 `access_log` 指令）
6. **TLS terminate 配置**：在 `server` block 中添加 `ssl_certificate` / `ssl_certificate_key` + `listen {port} ssl`
7. **TLS passthrough 配置**：使用 `stream` block + `ssl_preread on` + `proxy_pass` 按 SNI 路由
8. **WebSocket 配置**：添加 `proxy_set_header Upgrade $http_upgrade; proxy_set_header Connection "upgrade";`
9. **Access List 配置**：在 `server` 或 `location` block 中生成 `allow` / `deny` 指令，按 sort_order 排列，最后根据 default_policy 添加 `deny all` 或 `allow all`
10. **Custom headers**：按 op 类型生成 `proxy_set_header` (add/modify) 或 `proxy_hide_header` (delete)
11. **自定义 502 错误页面**：每个 HTTP server block 中添加 `proxy_intercept_errors on;` + `error_page 502 /502.html;` + 内部 location 指向 `nginx/html/502.html`。错误页面 HTML 在每次配置生成时由 `error_pages::write_error_pages()` 写入 `nginx/html/` 目录，使用与应用一致的视觉风格
12. **启动时生成配置**：应用启动时始终调用 `generate_all_configs`，不论 `auto_start_engine` 是否开启。确保 `nginx.conf` 在 status 轮询前就存在，避免首次安装时报错
13. **合成 HTTP→HTTPS 跳转块（`force_https`）**：对每个「至少有一条 `proxy_type=http` + `tls_mode=terminate` + `force_https=1` 规则」的域名，额外合成一个 80 端口 server block，内容只有 `listen 80;` + `server_name` + `return 308 https://$host$request_uri;`。它是**生成物**：不写 `proxy_rules`、不出现在 `list_proxies`、不参与 `detect_conflicts` 与规则计数。写在拥有该域名的 conf.d 文件末尾（同域多个 HTTPS 端口时只写在最小端口那个文件，保证只出现一次）；该域名已有真实 80 端口规则时跳过合成（避免重复 `server_name`），跳过原因由 `check_force_https` 暴露给表单做黄色提示
14. **默认安全响应头**：每个 HTTP server block 在 `server_name`/TLS 指令之后、`proxy_intercept_errors` 之前展开同一份默认头清单：`X-Content-Type-Options: nosniff`、`Referrer-Policy: strict-origin-when-cross-origin`、`X-Frame-Options: SAMEORIGIN`，每条都带 `always`（否则 502/404 等错误响应会丢头）。清单由 `security_headers::response_headers()` **单一函数**产出，server 块只有**一处**展开；`hostname_settings` 行可逐个关掉，没有行 = 三个全开。`location` 内禁止 `add_header`（1.26.2 子层只要出现一条就整体接管父层清单，`nginx -t` 仍会通过），502 的 internal location 保持零 `add_header`。`passthrough` 与 stream 规则不生成响应头，`http` 层也不写 `add_header`
15. **HSTS（`Strict-Transport-Security`）**：同一份头清单里的第四项，但生效条件有两个：该域名 `hostname_settings` 行 `hsts_enabled = 1`，**且**这个 server block 是 TLS 终止（`tls_mode = terminate`，即 `listen ... ssl`）。是否 TLS 只由 `tls_mode` 决定，**不看端口号**——443 + `tls_mode=none` 是完全合法的组合，不生成 HSTS。值为 `max-age=<秒>`（列默认 300），`hsts_include_subdomains = 1` 时追加 `; includeSubDomains`；因为值里带 `;`，生成时整段加引号。**不提供 preload**。每个 TLS server block 恰好一条；同一域名的多个 HTTPS 端口各自一个 server block，因此各自一条（HTTP 语义要求每个端点自己下发），而不是「每个域名一条」。没有行 = HSTS 关

## Nginx Config Generation Examples

### HTTP multi-domain same port
```nginx
# conf.d/port_80.conf
server {
    listen 80;
    server_name app.local;
    
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name api.local;
    
    location /v1 {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    location /graphql {
        proxy_pass http://127.0.0.1:4000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### HTTPS TLS terminate
```nginx
# conf.d/port_443.conf
server {
    listen 443 ssl;
    server_name app.local;
    ssl_certificate     /path/to/certs/cert_001.pem;
    ssl_certificate_key /path/to/certs/cert_001.key;
    
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Default security response headers
每个 HTTP server block（`tls_mode=none` 或 `terminate`）都带同一份默认头清单；`tls_mode=passthrough` 与 stream 规则不生成。`hostname_settings` 中某行把开关设为 0 时，只跳过对应的那一条。

```nginx
server {
    listen 443 ssl;
    server_name app.local;
    ssl_certificate     /path/to/certs/cert_001.pem;
    ssl_certificate_key /path/to/certs/cert_001.key;

    # Security response headers; `always` also covers error responses.
    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy strict-origin-when-cross-origin always;
    add_header X-Frame-Options SAMEORIGIN always;

    proxy_intercept_errors on;
    error_page 502 /502.html;
    location = /502.html {
        root "/path/to/data/nginx/html";
        internal;
    }
}
```

- 清单由 `security_headers::response_headers()` 产出（列 → 头名/值的映射集中在该模块），server 块只有一处循环展开
- 所有 `add_header` 都带 `always`，所以 502 错误页也带头；`location` 段（含 `/502.html`）内 `add_header` 计数必须为 0
- 没有 `hostname_settings` 行 = 三个头全开、HSTS 关；表行可以逐个关闭、HSTS 可以单独打开
- HSTS 只出现在 TLS 终止的 server block 上，普通 HTTP 块出现次数必须为 0；每个 TLS 块恰好 1 次（`test_support::assert_hsts_only_in_tls_server_blocks` 兜底）
- HSTS 值必须加引号（`max-age=300; includeSubDomains` 里的 `;` 否则会截断指令），且永远不含 `preload`

### HSTS on a TLS-terminating server

```nginx
server {
    listen 443 ssl;
    server_name app.local;

    # Security response headers; `always` also covers error responses.
    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy strict-origin-when-cross-origin always;
    add_header X-Frame-Options SAMEORIGIN always;
    add_header Strict-Transport-Security "max-age=300" always;
    ...
}
```

- 开关按 hostname 存：同域名的多条规则共享一行，表单只在「拥有者规则」上显示开关（拥有者 = 该域名启用中、TLS 终止、端口最小的一条规则，端口相同按规则 id 定序），其他规则显示只读提示
- 关闭开关 = 行里 `hsts_enabled = 0`，不生成 `max-age=0` 清理头；浏览器缓存不会被删除，这一点在 UI 里提示

### Synthesised HTTP -> HTTPS redirect (`force_https`)
`force_https` 不产生规则行：生成时对每个「存在 `proxy_type=http` + `tls_mode=terminate` + `force_https=1` 规则」的域名合成一个 80 端口 server block，追加在**该域名自己的 conf.d 文件**里（不改 `{port}_{domain}.conf` 命名，443 规则就落在 `443_app_local.conf`）。

```nginx
# conf.d/443_app_local.conf —— 紧跟该域名的 443 server block 之后
# Auto-generated by Meridian: HTTP -> HTTPS redirect. Do not edit manually.
server {
    listen 80;
    server_name app.local;
    return 308 https://$host$request_uri;
}
```

- 308 让客户端在跟随 Location 时保留请求方法与请求体（`return` 直接响应，不做内部跳转）
- 排除条件：该域名已有真实 80 端口规则，或产生跳转的规则自身就监听 80
- 关闭开关后重新生成即无残块：`conf.d/` 每次生成前清空，跳转块没有独立文件也没有数据库行
- 该块同样要能被 `nginx -t` 接受（集成测试用仓库自带的 sidecar 验证）

### TLS passthrough (SNI-based)
```nginx
# stream.d/stream_tls_pass_{id}.conf
stream {
    map $ssl_preread_server_name $upstream {
        app.local 127.0.0.1:3443;
    }
    server {
        listen 8443;
        ssl_preread on;
        proxy_pass $upstream;
    }
}
```

### TCP Stream
```nginx
# stream.d/stream_{id}.conf
server {
    listen 15432;
    proxy_pass 192.168.1.50:5432;
}
```

### With Access List
```nginx
location / {
    allow 192.168.1.0/24;
    allow 10.0.0.0/8;
    deny all;
    
    proxy_pass http://127.0.0.1:3000;
}
```

### With WebSocket
```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

### Per-Rule Logging (HTTP)
```nginx
location / {
    access_log "/path/to/data/nginx/logs/rule_abc123.access.log" meridian;
    access_log "/path/to/data/nginx/logs/access.log";
    proxy_pass http://127.0.0.1:3000;
}
```

### Per-Rule Logging (Stream)
```nginx
server {
    listen 15432;
    access_log "/path/to/data/nginx/logs/rule_abc123.access.log" stream_meridian;
    proxy_pass 192.168.1.50:5432;
}
```

### Custom 502 Error Page
```nginx
server {
    listen 80;
    server_name app.local;

    proxy_intercept_errors on;
    error_page 502 /502.html;
    location = /502.html {
        root "/path/to/data/nginx/html";
        internal;
    }

    location / {
        proxy_pass http://127.0.0.1:3000;
    }
}
```

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | Single HTTP rule, port 80 | Valid `port_80.conf` with 1 server block | |
| TP-002 | Normal | Two HTTP rules, same port, different domains | Single `port_80.conf` with 2 server blocks | Multi-domain aggregation |
| TP-003 | Normal | Two HTTP rules, same port + same domain, different path | Single server block with 2 location blocks | Path merging |
| TP-004 | Normal | TCP stream rule | Valid `stream_{id}.conf` | |
| TP-005 | Normal | HTTPS TLS terminate rule | ssl directives present in server block | |
| TP-006 | Normal | TLS passthrough rule | stream block with ssl_preread on | |
| TP-007 | Normal | Rule with access list (deny default) | allow/deny directives + `deny all` at end | |
| TP-008 | Normal | Rule with WebSocket enabled | Upgrade + Connection headers present | |
| TP-009 | Normal | Rule with custom headers | proxy_set_header / proxy_hide_header present | |
| TP-010 | Normal | Disabled rule in rule set | Disabled rule not present in any config file | |
| TP-011 | Error | Port conflict: two TCP streams on same port | `validate_port_conflicts` returns conflict | |
| TP-012 | Error | Port conflict: HTTP + Stream on same port | Conflict returned | |
| TP-013 | Error | Duplicate route: same domain + port + path_prefix | Conflict returned | |
| TP-014 | Normal | check_port_conflict with exclude_id | Self excluded from conflict check | |
| TP-015 | Normal | write_configs + successful nginx -t | New configs in place | |
| TP-016 | Error | write_configs + nginx -t failure | Previous configs restored (rollback) | |
| TP-017 | Boundary | 0 enabled rules | Empty conf.d/, empty stream.d/, valid main nginx.conf | |
| TP-018 | Normal | Main nginx.conf includes correct paths | http { include conf.d/*.conf } + stream { include stream.d/*.conf } | |
| TP-019 | Combination | Mix of HTTP + HTTPS + Stream + disabled rules | All correct configs, disabled skipped, aggregation correct | |
| TP-020 | Normal | HTTP server block generation | Contains `proxy_intercept_errors on` + `error_page 502` + internal location | 502 error page |
| TP-021 | Normal | `write_error_pages()` called during config generation | `nginx/html/502.html` file exists with valid HTML | |
| TP-022 | Normal | `worker_processes` set to "auto" | `nginx.conf` contains `worker_processes auto;` | |
| TP-023 | Normal | `worker_processes` set to "4" | `nginx.conf` contains `worker_processes 4;` | |
| TP-024 | Boundary | `worker_processes` set to invalid string | Falls back to `worker_processes 2;` | |
| TP-025 | Normal | App startup without auto-start | `nginx.conf` still generated (exists on disk) | First-launch fix |
| TP-026 | Normal | HTTPS rule with `force_https` on, no 80 rule for the domain | Owning conf.d file ends with a `listen 80` server block that `return 308`s | Ticket #22 |
| TP-027 | Normal | HTTPS rule with `force_https` on, domain already has a real 80 rule | No redirect block anywhere in conf.d/ | Skipped, form warns |
| TP-028 | Normal | Turn `force_https` off and regenerate | No `return 308` / `listen 80` residue | No separate file to clean up |
| TP-029 | Normal | `force_https` on, two HTTPS ports for one domain | Redirect block appears exactly once (lowest port file) | No duplicate server_name |
| TP-030 | Normal | HTTP server block, no `hostname_settings` row | 3 `add_header`, every one ends in `always` | Ticket #20 |
| TP-031 | Normal | `hostname_settings` row with `sec_nosniff = 0` | Only Referrer-Policy and X-Frame-Options, server block still valid | Ticket #20 |
| TP-032 | Normal | Passthrough rule and stream rule | Neither config contains `add_header`; http-level `nginx.conf` neither | Ticket #20 |
| TP-033 | Boundary | Any generated location, including `location = /502.html` | `add_header` count inside locations is 0 | Ticket #20 |
| TP-034 | Normal | TLS rule for a hostname with `hsts_enabled = 1` | Exactly one `add_header Strict-Transport-Security "max-age=…" always` in the server block | Ticket #23 |
| TP-035 | Normal | Plain HTTP rule for the same hostname with `hsts_enabled = 1` | No `Strict-Transport-Security` in that block | TLS decides, not the row alone |
| TP-036 | Boundary | `listen 443` with `tls_mode = none`, HSTS on | No HSTS header | Never judge TLS by the port |
| TP-037 | Normal | `hsts_max_age = 31536000`, `hsts_include_subdomains = 1` | Value is `"max-age=31536000; includeSubDomains"`; never contains `preload` | Ticket #23 |
| TP-038 | Combination | One hostname on two TLS ports plus one plain port, HSTS on | Each TLS block has exactly 1 HSTS, the plain block 0 | Invariant helper `assert_hsts_only_in_tls_server_blocks` |
| TP-039 | Integration | Generated config with HSTS handed to the repository sidecar | `nginx -t` accepts the quoted value | Ticket #23 |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Main config generation | `src-tauri/src/config_engine/main_config.rs` | `generate_main_config()` | Includes meridian + stream_meridian log_format |
| HTTP config generation | `src-tauri/src/config_engine/http_config.rs` | `generate_server_block()` | Per-rule + global access_log in each location |
| Redirect block generation | `src-tauri/src/config_engine/http_config.rs` | `generate_https_redirect_block()` | Synthesised 80 port `return 308`; `force_https_redirect_hostnames()` / `check_force_https()` decide when it applies |
| Stream config generation | `src-tauri/src/config_engine/stream_config.rs` | `generate_stream_block()` | Per-rule access_log with stream_meridian format |
| Error page generation | `src-tauri/src/config_engine/error_pages.rs` | `write_error_pages()` | Writes `nginx/html/502.html` |
| Security response headers | `src-tauri/src/config_engine/security_headers.rs` | `response_headers()` | Single source of the header list; column → name/value mapping lives here, HSTS included |
| HSTS ownership check | `src-tauri/src/config_engine/hsts.rs` | `owner_rule()`, `check()` | Lowest TLS port owns the hostname's switch; backs `get_hostname_hsts` / `set_hostname_hsts` / `list_hsts_hostnames` |
| Hostname settings overview | `src-tauri/src/config_engine/hostname_settings.rs` | `overview()` | Defines the list's orphan (no rule, enabled or disabled, names the hostname); cleanup applies the same predicate in SQL |
| Config orchestration | `src-tauri/src/config_engine/mod.rs` | `generate_all_configs()`, `generate_all_configs_with_settings()` | Passes data_dir, worker_processes and the hostname settings map to sub-generators |
| Port conflict detection | `src-tauri/src/config_engine/mod.rs` | `validate_port_conflicts()` | |
| Config write + rollback | `src-tauri/src/config_engine/mod.rs` | `write_configs()`, `restore_previous_configs()` | |
