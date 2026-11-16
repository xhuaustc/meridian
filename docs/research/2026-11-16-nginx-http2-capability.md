# Meridian sidecar nginx 1.26.2 能力调研：HTTP/2、gzip、brotli

- 日期：2026-11-16
- 范围：Meridian 随应用分发的 nginx sidecar（版本锁定 1.26.2）能否支撑「HTTPS 安全基线」所需的 HTTP/2、gzip、brotli
- 方法：只读。先读仓库构建脚本与 CI 配置，再逐条对照 nginx 官方文档、nginx 官方 CHANGES、nginx 官方 trac、ngx_brotli 官方仓库
- 关键前提：**macOS/Linux 与 Windows 的 sidecar 不是同一个构建**。Unix 是源码编译（可控制 configure 参数），Windows 是直接下载 nginx.org 官方预编译二进制（完全不可控制 configure 参数）。所以下面的结论必须分平台看。

---

## 一、结论速览

一句话：**这份 sidecar 在 macOS/Linux 上既没有 HTTP/2 能力也没有 gzip，更没有 brotli；三项全部需要改构建。** Windows 走官方二进制，情况与 Unix 不同且部分无法从一手来源确认。

| # | 项目 | macOS / Linux（源码构建） | Windows（nginx.org 官方二进制） |
|---|------|---------------------------|--------------------------------|
| 1 | HTTP/2（`http2 on;` 或 `listen ... http2`） | **需要改构建**：缺 `--with-http_v2_module` | **无法从一手来源确认**（官方文档称功能「几乎等同 Unix」，未列模块清单）→ 需 `nginx -V` 实测 |
| 1a | `listen 443 ssl http2` 语法 | 1.26.2 仍接受，但自 1.25.1 起已 deprecated，不建议新写 | 同上 |
| 1b | `http2 on;` 语法 | 1.26.2 支持该指令（1.25.1 引入），但需模块被编进去 | 同上 |
| 1c | 是否 TLS-only | **否**，nginx 支持无 TLS 的 HTTP/2（h2c）；走 TLS 时需要 ALPN（OpenSSL ≥ 1.0.2，本构建是 3.3.2） | 同 |
| 2 | gzip（`ngx_http_gzip_module`） | **不支持**：脚本显式 `--without-http_gzip_module`（需改构建删掉这一行） | **无法从一手来源确认**（gzip 是 nginx 默认编入的模块，推定可用） |
| 3 | brotli（`ngx_brotli`） | **不支持**：配置里没有任何 `--add-module` / `--add-dynamic-module`，也没下载 ngx_brotli 源码（需改构建） | **不支持**（第三方模块，官方二进制不含；推断，见 E7） |
| 4 | HTTP/2 与 stream 块的关系 | **确认无关**：stream 的 `listen` 没有 `http2` 参数，`http2` 指令只在 `http`/`server` 上下文 | 同 |
| 5 | HTTP/2 下的 WebSocket（RFC 8441） | **不支持**，仍需回落 HTTP/1.1 | 同（这是 nginx 核心能力问题，与平台/构建无关） |

---

## 二、逐条结论与依据

### 1. HTTP/2 —— 需要改构建（macOS/Linux）

**结论：不支持。** 源码构建的 configure 参数里没有 `--with-http_v2_module`，`ngx_http_v2_module` 不会被编译进去。因此无论是 `listen 443 ssl http2` 还是 `http2 on;`，在 macOS/Linux 的 sidecar 上都会直接导致配置校验失败（unknown directive）。

**依据（仓库）：** `scripts/prepare-nginx.sh` 第 117–167 行是完整的 `CONFIGURE_ARGS`，启用的模块只有：

```
--with-http_ssl_module          (第 131 行)
--with-stream                   (第 132 行)
--with-stream_ssl_module        (第 133 行)
--with-stream_ssl_preread_module (第 134 行)
```

其中**没有** `--with-http_v2_module`；第 169 行直接 `./configure "${CONFIGURE_ARGS[@]}"`，没有其他追加参数。全仓库检索 `http2` / `http_v2` 在构建脚本与 Rust 源码中均无命中（只有 `Cargo.lock` 里的无关第三方 crate）。

**依据（官方文档）：** ngx_http_v2_module 页面明确写着该模块「is not built by default, it should be enabled with the `--with-http_v2_module` configuration parameter」，且 `configure.html` 的 `--with-http_v2_module` 条目同样标注「This module is not built by default」。所以它是纯编译期开关，不是配置期开关 —— 这正是本构建缺的那一项。

- <https://nginx.org/en/docs/http/ngx_http_v2_module.html>
- <https://nginx.org/en/docs/configure.html>

**两种写法的现状（1.26.2）：** 官方 CHANGES 的 1.25.1 条目（2023-06-13）写明：「Feature: the "http2" directive, which enables HTTP/2 on a per-server basis; the "http2" parameter of the "listen" directive is now deprecated.」所以：

- `http2 on;`：1.25.1 起可用，是当前写法，1.26.2 支持（前提是模块编进去了）。
- `listen 443 ssl http2`：仍被接受，但已 deprecated；核心模块文档在 `listen` 参数列表里保留该参数并注明「The parameter is deprecated, the http2 directive should be used instead」。新代码不应再写它。

- <https://nginx.org/en/CHANGES-1.26>（1.25.1 段落）
- <https://nginx.org/en/docs/http/ngx_http_core_module.html#listen>

**是不是 TLS-only？不是。** `listen` 参数文档说该参数「configures the port to accept HTTP/2 connections. Normally, for this to work the `ssl` parameter should be specified as well, but nginx can also be configured to accept HTTP/2 connections without SSL」；`$http2` 变量的取值包含明文 `h2c`；CHANGES 1.25.3 还有一条「Bugfix: a segmentation fault might occur in a worker process when using HTTP/2 without SSL」，都说明无 TLS 的 HTTP/2 是被支持的模式。但走 TLS 时 HTTP/2 依赖 ALPN，文档明确「requires the "Application-Layer Protocol Negotiation" (ALPN) TLS extension support, which is available since OpenSSL version 1.0.2」——本构建静态链接 OpenSSL 3.3.2（`scripts/prepare-nginx.sh` 第 37、127 行），满足要求。

> 对「HTTPS 安全基线」的含义：HTTP/2 应当明确绑定在 TLS 监听上对外提供，同时不要指望用它承载 WebSocket（见第 5 条）。

**推进建议（需要改构建）：** 在 `CONFIGURE_ARGS` 里加 `--with-http_v2_module`；配置生成侧统一输出 `listen 443 ssl;` + `http2 on;`，不要用 deprecated 的 `listen ... http2`。

### 2. gzip —— 不支持（macOS/Linux），需要改构建

**结论：不支持。** `ngx_http_gzip_module` 是 nginx 默认编入的模块，但本构建**显式关掉了它**。

**依据（仓库）：** `scripts/prepare-nginx.sh` 第 138 行：`--without-http_gzip_module`（位于第 117–167 行的 `CONFIGURE_ARGS` 中，属于「Disable unused modules to minimize binary size」那一段，注释在第 136 行）。后果是 `gzip` / `gzip_types` / `gzip_min_length` 等指令在该二进制上全部不可用。

**依据（官方文档）：** `configure.html` 里 gzip 只以「关闭」开关的形式出现——`--without-http_gzip_module`「disables building a module that compresses responses of an HTTP server. The zlib library is required to build and run this module」。凡默认不编入的模块，官方都用 `--with-*` 形式描述并注明「not built by default」；gzip 只有 `--without-*`，即默认开启、可被关闭。

- <https://nginx.org/en/docs/configure.html>
- <https://nginx.org/en/docs/http/ngx_http_gzip_module.html>

**推进建议（需要改构建）：** 删掉第 138 行即可。成本极低：zlib 1.3.1 已经通过 `--with-zlib="../zlib-${ZLIB_VERSION}"` 静态链接（第 128 行），不需要新增依赖。

> 附带的安全提醒（官方文档原文）：gzip 页面注明「When using the SSL/TLS protocol, compressed responses may be subject to BREACH attacks.」如果「HTTPS 安全基线」里要默认开压缩，需要在文档里说明这一取舍（建议对含敏感 token 的响应关闭压缩或限制 `gzip_types`）。

### 3. brotli —— 不支持，需要改构建（两个平台都是）

**结论：不支持。** 构建脚本里没有任何外部模块接入点，也没有 ngx_brotli 源码。

**依据（仓库）：** `CONFIGURE_ARGS`（`scripts/prepare-nginx.sh` 第 117–167 行）中不含 `--add-module` 或 `--add-dynamic-module`；下载的三份源码只有 nginx、OpenSSL、zlib（第 68–70、101–103 行），没有 ngx_brotli。全仓库检索 `brotli` 在脚本中零命中（`src-tauri/Cargo.lock` 里的 brotli 是 Rust 侧依赖，与 nginx 无关）。

**依据（ngx_brotli 官方仓库）：** 它是第三方模块，README 的安装章节只有两种接入方式，且都必须显式传给 configure：静态编译用 `./configure --add-module=/path/to/ngx_brotli`，动态加载用 `./configure --with-compat --add-dynamic-module=/path/to/ngx_brotli`，并需要 `git clone --recurse-submodules`（依赖 `deps/brotli` 子模块）。

- <https://github.com/google/ngx_brotli>（README：Installation 章节）

**推进建议（需要改构建）：** 两条路，二选一：

1. 静态：把 ngx_brotli（含 `deps/brotli` 子模块）下载到 `.nginx-build/`，加 `--add-module=../ngx_brotli`。改动集中在 `prepare-nginx.sh` 的下载段与 `CONFIGURE_ARGS`。
2. 动态：当前构建**没有** `--with-compat`（第 117–167 行确认），所以动态路线必须同时加 `--with-compat`，否则按 ngx_brotli README 的说明会遇到「module is not binary compatible」。
3. Windows 侧无源码构建入口（见 E6），要 brotli 就得改成自建 Windows 二进制，成本明显更高。**建议先把 brotli 划到 v2 或明确标记「Unix only」。**

### 4. HTTP/2 与 stream 块无关 —— 确认

**结论：确认。** `http2` 指令的上下文是 `http`、`server`，不是 `stream`；stream 模块自己的 `listen` 指令参数表里根本没有 `http2`。

**依据（官方文档）：**

- `ngx_http_v2_module` 的 `http2` 指令：Syntax `http2 on | off;`，**Context: `http`, `server`**。
- `ngx_stream_core_module` 的 `listen` 语法：`listen address:port [default_server] [ssl] [udp] [proxy_protocol] [setfib=] [fastopen=] [backlog=] [rcvbuf=] [sndbuf=] [accept_filter=] [deferred] [bind] [ipv6only=] [reuseport] [multipath] [so_keepalive=]` —— 没有 `http2`。同一页面里 `$protocol` 变量的取值只有「`TCP` or `UDP`」。

- <https://nginx.org/en/docs/http/ngx_http_v2_module.html>
- <https://nginx.org/en/docs/stream/ngx_stream_core_module.html#listen>

**对落地的含义：** 如果「HTTPS 安全基线」要同时覆盖 http 代理规则与 stream（TLS 直通/SNI 分流）规则，基线配置必须分两套：stream 块只能动 `ssl_protocols` / `ssl_ciphers` / `ssl_session_*`（`--with-stream_ssl_module` 已启用，第 133 行）这类 TLS 参数，HTTP/2 只对 http 块有意义。

### 5. HTTP/2 下的 WebSocket（RFC 8441）—— 不支持，仍需回落 HTTP/1.1

**结论：不支持。** nginx 1.26.2 没有实现 RFC 8441（HTTP/2 上的 Extended CONNECT / WebSocket）。这是 nginx 核心能力问题，与本次构建开关无关，两平台一样。实际行为就是：客户端经 ALPN 协商到 h2 后无法在该连接上建 WebSocket，要么浏览器/客户端回落到 HTTP/1.1 建连，要么显式给 WebSocket 走 h1（例如单独用一个不带 `http2 on;` 的 server 或端口）。

**依据（一手来源）：**

1. nginx 官方 trac 上，这条需求 **#1992「Websocket over HTTP/2 support」至今仍是 open 状态（status: new）**，2020-06-02 提出，最后修改于 2024-05-12 且仍未实现。nginx 核心开发者 Maxim Dounin 的回复明确说明：(a) nginx 本身不实现 WebSocket 协议，只是把 HTTP/1.1 的 upgrade 代理过去；(b)「RFC 8441 defines a completely different way to establish websocket communication, using a CONNECT method」，要做就得把 CONNECT 翻译成 HTTP/1.1 Upgrade 或透传给 HTTP/2 后端，而「this is unlikely to happen unless there is a practical demand」。
   - <https://trac.nginx.org/nginx/ticket/1992>
2. nginx 官方 CHANGES 1.21.1（2021-07-06）：「Change: now nginx always returns an error for the CONNECT method.」——在 1.26.2 上 CONNECT 一律报错，RFC 8441 的基础方法根本走不通。（后续 1.31.0 才引入 `ngx_http_tunnel_module` 处理 HTTP/1.1 CONNECT，1.26.2 没有。）
   - <https://nginx.org/en/CHANGES-1.26>（1.21.1 与 1.30/1.31 相关条目）
3. 官方 WebSocket 代理文档通篇只描述 HTTP/1.1 的协议切换机制：「To turn a connection between a client and server from HTTP/1.1 into WebSocket, the protocol switch mechanism available in HTTP/1.1 is used」，示例就是 `proxy_set_header Upgrade $http_upgrade; proxy_set_header Connection "upgrade";`。
   - <https://nginx.org/en/docs/http/websocket.html>

**对落地的含义：** 文档和 UI 里不要承诺「HTTP/2 下 WebSocket 也能用」。基线默认开 HTTP/2 时，WebSocket 路径需要显式的 HTTP/1.1 回落说明，否则用户会以为是代理配置写错了。

---

## 三、无法从一手来源确认的点

1. **Windows 侧边车到底编了哪些模块。** `scripts/prepare-nginx.ps1` 只是下载并解包 nginx.org 的官方 zip（第 38、51–60、63–70 行），没有 configure 环节；nginx 官方也没有公布 Windows 二进制的 configure 参数清单。官方 windows 文档只说「it provides almost the same functionality as a UNIX version of nginx except for XSLT filter, image filter, GeoIP module, and embedded Perl language」——这暗示标准模块（含 http_v2、gzip）都在，但**没有逐项列清单**，所以不能据此断言 `--with-http_v2_module` 已编入。要确认只能在装好的 sidecar 上跑 `nginx -V` 看 configure arguments，或用一个含 `http2 on;` 的配置跑 `nginx -t`。
   - <https://nginx.org/en/docs/windows.html>
2. **Windows 侧是否含 brotli。** 官方包是 nginx 官方自行构建的，ngx_brotli 必须 `--add-module` 才存在（E4），因此推断不含；但这是「缺失证据 + 推断」，不是文档明文。
3. **`nginx -V` 实测未执行。** 本次调研按只读约束未编译、未运行 sidecar 二进制，所以上述 Unix 结论全部来自 configure 参数与实际模块归属的对应关系（模块文档 + configure 文档），而非运行期验证。建议实现该功能前补一次 `nginx -V` / `nginx -t` 冒烟。

---

## 四、顺带发现（与「HTTPS 安全基线」直接相关，未被问到但会影响方案）

1. **`return` 不可用 —— 这条影响 http→https 跳转。** 第 137 行有 `--without-http_rewrite_module`，而 `return`、`rewrite`、`if`、`set`、`break` 全部属于 `ngx_http_rewrite_module`。所以在当前 sidecar 上无法用 `return 301 https://...` 实现强制跳转。要么连同 `--with-http_v2_module` 一起把 rewrite 模块恢复（注意：configure 参数里没有 `--with-pcre`，恢复后会走系统 PCRE），要么在方案里明确「不做协议跳转」。
   - <https://nginx.org/en/docs/http/ngx_http_rewrite_module.html>
   - <https://nginx.org/en/docs/configure.html>
2. **HSTS 之类的响应头是可用的。** `add_header` 属于 `ngx_http_headers_module`，该模块在 `configure.html` 里没有对应的 `--without-` 开关，也不在第 117–167 行的禁用列表里，因此 `add_header Strict-Transport-Security ...` 可以正常写。
3. **TLS 参数可用。** `ssl_protocols` / `ssl_ciphers` / `ssl_prefer_server_ciphers` / `ssl_session_*` 属于 `ngx_http_ssl_module`，第 131 行已启用；stream 侧的对应指令由第 133 行的 `--with-stream_ssl_module` 覆盖。
4. **构建缓存会让「改构建」看起来没生效。** `release.yml` 第 128 行的缓存 key 包含 `hashFiles('scripts/prepare-nginx.sh', 'scripts/prepare-nginx.ps1')`，所以改脚本会自然失效缓存；但本地 `.nginx-build/` 与 `src-tauri/binaries/nginx-*` 已存在时脚本会提前退出（`prepare-nginx.sh` 第 49–54 行），本地验证记得 `FORCE_REBUILD=1`。

---

## 五、证据清单

| 编号 | 来源 | 位置 / URL | 支撑的结论 |
|------|------|------------|-----------|
| E1 | `scripts/prepare-nginx.sh` | 第 36 行 `NGINX_VERSION="${NGINX_VERSION:-1.26.2}"` | 版本基线 1.26.2 |
| E2 | `scripts/prepare-nginx.sh` | 第 117–167 行 `CONFIGURE_ARGS`；第 131–134 行启用的模块；无 `--with-http_v2_module` | HTTP/2 缺失 |
| E3 | `scripts/prepare-nginx.sh` | 第 138 行 `--without-http_gzip_module`；第 128 行 `--with-zlib` | gzip 被关闭，zlib 已在 |
| E4 | `scripts/prepare-nginx.sh` + ngx_brotli README | 脚本第 117–167 行无 `--add-module`；<https://github.com/google/ngx_brotli> | brotli 完全未接入 |
| E5 | `.github/workflows/release.yml` | 第 22–25 行 env（`NGINX_VERSION: '1.26.2'`）；第 130–135 行 Unix 源码构建；第 137–142 行 Windows 调用 ps1 | 平台分支 |
| E6 | `scripts/prepare-nginx.ps1` | 第 38 行 zip URL、第 51–60 行下载解包、第 63–70 行只拷 `nginx.exe` | Windows 无 configure 控制 |
| E7 | nginx 官方 windows 文档 | <https://nginx.org/en/docs/windows.html> | Windows 版「功能几乎等同 Unix，仅缺 4 项」，但无模块清单 |
| E8 | nginx 官方 configure 文档 | <https://nginx.org/en/docs/configure.html> | v2 模块默认不编入；gzip 只有 `--without` 开关 |
| E9 | ngx_http_v2_module 文档 | <https://nginx.org/en/docs/http/ngx_http_v2_module.html> | 模块默认不编入；`http2` 指令 context 为 `http, server`；ALPN/OpenSSL ≥1.0.2 |
| E10 | ngx_http_core_module `listen` | <https://nginx.org/en/docs/http/ngx_http_core_module.html#listen> | `http2` 参数 1.9.5 引入、已 deprecated、可无 SSL 运行 |
| E11 | nginx CHANGES 1.26 分支 | <https://nginx.org/en/CHANGES-1.26> | 1.25.1 引入 `http2` 指令并 deprecate `listen ... http2`；1.25.3「HTTP/2 without SSL」修复；1.21.1 CONNECT 一律报错；1.26.2 仅 mp4 安全修复 |
| E12 | ngx_stream_core_module | <https://nginx.org/en/docs/stream/ngx_stream_core_module.html#listen> | stream `listen` 无 `http2` 参数；`$protocol` 仅 TCP/UDP |
| E13 | nginx trac #1992 | <https://trac.nginx.org/nginx/ticket/1992> | RFC 8441 未实现（ticket 仍 open，核心开发者明确说明） |
| E14 | WebSocket 代理文档 | <https://nginx.org/en/docs/http/websocket.html> | 官方只描述 HTTP/1.1 Upgrade 方案 |
| E15 | ngx_http_rewrite_module | <https://nginx.org/en/docs/http/ngx_http_rewrite_module.html> | `return`/`rewrite`/`if`/`set` 属于该模块 → 第 137 行禁用后不可用 |

---

## 六、建议的最小改构建清单（供后续实现参考）

1. `--with-http_v2_module`（macOS/Linux）——配置侧统一用 `http2 on;`。
2. 删 `--without-http_gzip_module`（macOS/Linux）——zlib 已静态链接，无新增依赖。
3. brotli：先不做，或静态 `--add-module`（需新增源码下载 + 子模块）；不做动态，因为缺 `--with-compat`。
4. 评估是否恢复 `--without-http_rewrite_module` 关掉的能力（影响 http→https 跳转）。
5. Windows 侧先跑一次 `nginx -V` 落定真实模块清单，再决定基线功能是否需要在 Windows 上降级。
