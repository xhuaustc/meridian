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
- **Input:** `(data_dir, rules, certs, access_lists, worker_processes: &str)`
- **Output:** 同 `generate_all_configs`
- **Notes:** 支持传入 `worker_processes` 设置值（"auto" 或数字字符串）。由 `apply_config_inner` 和应用启动流程调用，从数据库读取 `worker_processes` 设置。

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

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Main config generation | `src-tauri/src/config_engine/main_config.rs` | `generate_main_config()` | Includes meridian + stream_meridian log_format |
| HTTP config generation | `src-tauri/src/config_engine/http_config.rs` | `generate_server_block()` | Per-rule + global access_log in each location |
| Stream config generation | `src-tauri/src/config_engine/stream_config.rs` | `generate_stream_block()` | Per-rule access_log with stream_meridian format |
| Error page generation | `src-tauri/src/config_engine/error_pages.rs` | `write_error_pages()` | Writes `nginx/html/502.html` |
| Config orchestration | `src-tauri/src/config_engine/mod.rs` | `generate_all_configs()`, `generate_all_configs_with_settings()` | Passes data_dir + worker_processes to sub-generators |
| Port conflict detection | `src-tauri/src/config_engine/mod.rs` | `validate_port_conflicts()` | |
| Config write + rollback | `src-tauri/src/config_engine/mod.rs` | `write_configs()`, `restore_previous_configs()` | |
