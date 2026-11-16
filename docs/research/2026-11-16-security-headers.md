# 反向代理场景下的默认安全响应头调研

- 日期：2026-11-16
- 状态：调研（只读，未改代码）
- 场景：Meridian 为用户站点生成 nginx 反向代理配置（`src-tauri/src/config_engine/http_config.rs` 的 `generate_server_block`），计划「默认加一组安全响应头，用户可选择开启」
- 一手来源范围：nginx 官方文档与官方源码、MDN、OWASP Cheat Sheet Series、RFC 6797 / RFC 7034、WHATWG Fetch 规范、W3C CSP3 / Referrer Policy 规范、hstspreload.org（Chrome HSTS preload 列表的官方站点）

本文回答三个问题：(1) 每个头该不该做默认值、取值与风险；(2) 上游已有同名头时 `add_header` 是覆盖还是追加、`always` 干什么；(3) `add_header` 的继承陷阱。

除了文档与规范，nginx 层面的结论还在 **本仓库打包的那个 nginx 二进制**上做了实测（`src-tauri/binaries/nginx-aarch64-apple-darwin`，`nginx/1.26.2`，见第 5 节），因为「文档说追加」与「实际线上根因」之间常有出入。

---

## 1. 结论总表

| 响应头 | 推荐默认值 | 主要风险 | 是否建议默认开 |
| --- | --- | --- | --- |
| `X-Content-Type-Options` | `nosniff` | 上游把 JS/CSS 以错误 MIME（如 `text/plain`）提供时，脚本与样式会被浏览器直接阻断（Fetch 规范行为）；上游 `Content-Type` 配错的老站点可能「忽然样式丢了」 | **建议默认开**（风险最低、收益明确） |
| `X-Frame-Options` | `SAMEORIGIN`（`DENY` 更严但更易砸） | 阻止一切跨源嵌入：跨子域 iframe（`app.example.com` 嵌 `auth.example.com`）、第三方面板/预览类嵌入会白屏；`DENY` 连站内自嵌也禁 | **建议默认开**（`SAMEORIGIN`），但必须在 UI 上可关；有 CSP `frame-ancestors` 的上游会被重复头干扰 |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | 跨源只发 origin（无路径/查询串）：依赖完整 `Referer` 做防盗链、跨站单点登录回调、渠道分析的站点会受影响；与现实浏览器默认值相同，破坏面最小 | **建议默认开**（等价于显式写出浏览器默认值，几乎不改变行为） |
| `Permissions-Policy` | 不建议给默认值；若要给，只给保守子集，如 `geolocation=(), camera=(), microphone=()` | 关掉 `camera`/`microphone` 直接废掉 WebRTC、扫码、录音类应用；`fullscreen=()` 会废掉视频全屏这类基础功能；MDN 至今标注该头「Limited availability / experimental」 | **建议默认关**，用户显式开启并自选清单 |
| `Content-Security-Policy` | **不给默认值** | 任何默认策略都会砸站：`default-src 'self'` 阻断 CDN、内联 `<script>`/`<style>`、`img-src data:`、前端框架的 eval；且与上游自己的 CSP 叠加时按交集收紧，只会更难通过 | **必须用户显式开启**（默认关） |
| `Strict-Transport-Security` | 默认关；开启时建议 `max-age=300`（或 86400）起步、**不带** `includeSubDomains`、**不带** `preload`；只在 TLS 终止的 server 块里发 | 证书过期/换域名/回退 HTTP 期间用户被锁死（浏览器不给「继续访问」）；`includeSubDomains` 波及所有子域；`preload` 基本不可逆；HTTP 响应上的 HSTS 被浏览器忽略（等于白加） | **默认关**，且必须「仅 HTTPS 站点」才允许开启 |

一句话取舍：**`X-Content-Type-Options` / `Referrer-Policy` / `X-Frame-Options` 可以默认为开，`Permissions-Policy` / `CSP` / `HSTS` 必须默认关**。前三个是「几乎不改变页面能力」的加固，后三个是「改变站点能力边界」的策略。

---

## 2. 逐头说明

### 2.1 `X-Content-Type-Options: nosniff`

- **语义**：告诉浏览器按 `Content-Type` 声明处理响应，不要嗅探。（[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/X-Content-Type-Options)）
- **规范行为（这是「风险」的来源，不是猜测）**：启用 nosniff 后，`destination` 为 `script` 的响应若 MIME 不是 JavaScript MIME 类型、为 `style` 的响应若 MIME 不是 `text/css`，请求被阻断；其余响应类型不再嗅探。（[Fetch 规范 §3.6 `X-Content-Type-Options`](https://fetch.spec.whatwg.org/#x-content-type-options-header)，规范里取 `values[0]` 判断是否 `nosniff`）
- **为什么适合做默认值**：不改动页面可加载的资源能力，只把「MIME 说错但内容能跑」的运气路径关掉；OWASP 直接给 `X-Content-Type-Options: nosniff` 作为推荐值，并要求「全站正确设置 `Content-Type`」。（[OWASP HTTP Security Response Headers Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）
- **风险**：上游用错 MIME 提供 JS/CSS 的老应用会立刻「白屏/无样式」，而错误现象出现在用户站点上、根因却在 Meridian 生成的配置里，排障成本高。建议在 UI 上标注「若站点出现脚本/样式加载失败，可关闭此头」。
- **判定**：默认开。

### 2.2 `X-Frame-Options`

- **取值**：`DENY` / `SAMEORIGIN`；`ALLOW-FROM` 已废弃，现代浏览器遇到会忽略整个头。（[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/X-Frame-Options)）
- **规范**：三个值互斥，「头字段必须被设置成三者中恰好一个」。（[RFC 7034 §2.1](https://www.rfc-editor.org/rfc/rfc7034#section-2.1)）
- **OWASP 推荐**：优先用 CSP `frame-ancestors`；若用 XFO 则「不允许在 frame 中显示」→ `X-Frame-Options: DENY`。同时 OWASP 指出：**对重定向响应或返回 JSON 的 API 响应，XFO 没有安全价值**。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）
- **为什么默认值建议 `SAMEORIGIN` 而不是 OWASP 的 `DENY`**：`SAMEORIGIN` 只在跨**源**嵌入时拦截，站内自嵌（同源 iframe、同源编辑器预览）仍然可用；`DENY` 连自家页面嵌自家页面都禁，对「用户自定义站点」这类不可知目标更容易砸。注意 `SAMEORIGIN` 比较的是 origin，`app.example.com` 嵌 `auth.example.com` 属于跨源，会被拦——这是最常见的「开了就白屏」来源。（RFC 7034 §2.1 亦说明各实现对 SAMEORIGIN 的判定曾有差异：「current implementations vary on the interpretation of this criteria」）
- **与其他机制的关系**：CSP `frame-ancestors` 覆盖并取代 XFO 的能力；MDN 在上页明确提示用 CSP 得到更细粒度控制。（[MDN `frame-ancestors`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/frame-ancestors)）
- **判定**：默认开 `SAMEORIGIN`，可关。若上游已自带 CSP `frame-ancestors`，默认值会造成重复头（见 3.4）。

### 2.3 `Referrer-Policy: strict-origin-when-cross-origin`

- **语义**：同源请求发完整 URL（origin + path + query）；跨源请求只发 origin，且从 HTTPS 到 HTTP 时完全不发。（[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy)）
- **关键事实**：这**已经**是浏览器在没有策略时的默认值（自 2020-11 规范修订起；此前默认是 `no-referrer-when-downgrade`）。（[MDN 同页「Note: This is the default policy if no policy is specified」](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy)）
- **OWASP 推荐**：`Referrer-Policy: strict-origin-when-cross-origin`，并明确「Set the header explicitly rather than relying on browser defaults」。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）
- **风险**：因为与现行浏览器默认值相同，做默认值几乎不改变行为，收益是「显式化 + 通过安全评分工具」。真正会改变的只有两种情况：把值改成更严的 `no-referrer` / `same-origin`（依赖 `Referer` 的防盗链、SSO 回跳、来源分析会失效），或改成 `unsafe-url`（泄露路径，MDN 明确警告）。
- **补充**：该头还影响 `Origin` 头在 `no-referrer` 等策略下被置为 `null`（主要作用于表单提交等 navigate 类请求）。（[MDN 同页](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy)）
- **判定**：默认开（与浏览器默认同值，属于「写出来更清楚」）。

### 2.4 `Permissions-Policy`

- **语义**：控制文档及其 iframe 中浏览器特性的可用性，语法为 `feature=(allowlist)`，多个指令逗号分隔。（[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Permissions-Policy)）
- **成熟度**：MDN 把该头标为 **Limited availability / experimental**：「This feature is not Baseline because it does not work in some of the most widely-used browsers.」（[MDN 同页](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Permissions-Policy)）
- **OWASP 推荐**：`Permissions-Policy: geolocation=(), camera=(), microphone=()`，即禁用站点不需要的特性。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）
- **风险**：这不是「加固」，而是「削减能力」。反向代理后面是不可知的用户应用：视频会议站被 `camera=()` 废掉、扫码登录被 `camera=()` 废掉、HTML5 视频被 `fullscreen=()` 废掉都是常见事故。且特性名会随规范演进，写死的清单有长期维护成本。
- **判定**：默认关；开启时只给保守子集（`geolocation` / `camera` / `microphone`），不要给 `*` 之外的全量清单，更不要抄「禁用所有特性」。

### 2.5 `Content-Security-Policy`（默认给会不会砸坏用户站点）

**会。默认给 CSP 等于替用户应用决定「它能加载什么」，而代理层不可能知道上游应用依赖哪些内联脚本、CDN、`data:` 图片与 WebSocket。**

- **破坏机理（规范层面）**：CSP 的 fetch 指令限制资源加载来源，`script-src` 不含 `'unsafe-inline'` 时内联脚本被拒；`default-src 'self'` 会同时作为其余 fetch 指令的兜底。（[MDN CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy)）
- **叠加机理（比「砸站」更隐蔽）**：**多条 CSP 不是覆盖而是全部生效**。MDN：「The CSP mechanism allows multiple policies being specified for a resource… Adding additional policies can only further restrict the capabilities of the protected resource」；规范同义：「when multiple policies are present, each must be enforced or reported, according to its type」。（[MDN §Multiple content security policies](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy#multiple_content_security_policies)、[W3C CSP3 §8.1 The effect of multiple policies](https://www.w3.org/TR/CSP3/#multiple-policies)）
  → 若上游应用已发 CSP，代理再补一条「看起来无害」的 CSP，实际效果是**交集收紧**，会把本来能跑的页面打断，而且报错只出现在浏览器 console 里。
- **OWASP 的态度**：CSP「complex to configure and maintain」，且「对不渲染的 REST API 响应可能没有意义」。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）
- **正确做法**：默认关；开启时提供模板 + 明确警告 + 「先 Report-Only 观察」的路径（`Content-Security-Policy-Report-Only`，同样会与既有策略叠加，但只报告不阻断）。
- **判定**：必须显式开启，绝不做默认值。

### 2.6 `Strict-Transport-Security`（只讲事实与默认建议）

**事实：**

1. 只在 **HTTPS 响应**上有效：`If an HTTP response is received over insecure transport, the UA MUST ignore any present STS header field(s).`（[RFC 6797 §8.1](https://www.rfc-editor.org/rfc/rfc6797#section-8.1)）→ 写在 80 端口 server 块里等于没写。
2. 同一个响应里出现多个 STS 头时，UA 只处理**第一个**：`If a UA receives more than one STS header field in an HTTP response message over secure transport, then the UA MUST process only the first such header field.`（RFC 6797 §8.1）→ 与应用自带的 HSTS 冲突时，「谁排前面谁生效」（nginx 的排布见 3.4 实测）。
3. HSTS 策略作用于**该主机的所有端口**，与请求端口无关：`UAs apply HSTS Policy whenever making an HTTP connection to a Known HSTS Host, regardless of host port number; i.e., it applies to all ports on a Known HSTS Host`（RFC 6797 附录 B「Differences between HSTS Policy and Same-Origin Policy」）；浏览器做升级时 80→443，其它显式端口保留端口号但改用 HTTPS。（[RFC 6797](https://www.rfc-editor.org/rfc/rfc6797)、[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Strict-Transport-Security)）
4. IP 地址不能成为 HSTS host：`An IP address cannot be an HSTS host`；证书报错时浏览器不再提供「继续访问」的选项。（[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Strict-Transport-Security)）
5. `max-age=0` 只有在浏览器**再次通过 HTTPS** 访问并收到响应后才生效——也就是说「写错了想撤回」没有即时手段。（[MDN 同页](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Strict-Transport-Security)）
6. 预加载列表要求：`max-age` ≥ `31536000`、`includeSubDomains`、`preload`、所有子域都要能过 HTTPS；`preload` 指令本身即视为「请求加入列表」。（[hstspreload.org §Submission Requirements](https://hstspreload.org/)）

**默认建议：默认关。**

理由不是「HSTS 不好」，而是**它把一次配置错误变成用户侧不可逆故障**：证书过期/吊销、域名换绑、临时回退 HTTP、子域被 `includeSubDomains` 扫到，都会让用户「打不开且无法点过」。OWASP 也把这条风险写在小节开头：「If the HSTS header is misconfigured or if there is a problem with the SSL/TLS certificate being used, legitimate users might be unable to access the website」。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）

若要提供开关，默认值建议：

- 只允许在 **TLS 终止**的 server 块生效（Meridian 的 `tls_mode=terminate`）；`passthrough`（stream 层）根本看不到 HTTP，无法加头（见 5.2）。
- 默认 `max-age=300` 或 `86400`，**不带** `includeSubDomains`、**不带** `preload`。
- 用提示引导用户按「5 分钟 → 1 周 → 1 月」逐步增大，这是 preload 站点给出的官方 ramp-up 建议：`max-age=300; includeSubDomains` → `604800` → `2592000`，每阶段观察站点指标。（[hstspreload.org §Deployment Recommendations](https://hstspreload.org/)）

---

## 3. 上游已设置同名响应头时，`add_header` 是覆盖还是追加？

**结论：追加，不覆盖。** 同名头会变成**两个**字段同时发回客户端。要「覆盖」必须先 `proxy_hide_header` 把上游的那个丢掉，再 `add_header` 补自己的。

### 3.1 官方文档依据

- `add_header`：「Adds the specified field to a response header provided that the response code equals 200, 201 (1.3.10), 204, 206, 301, 302, 303, 304, 307 (1.1.16, 1.0.13), or 308 (1.13.0).」（[nginx docs: ngx_http_headers_module / add_header](https://nginx.org/en/docs/http/ngx_http_headers_module.html#add_header)）——语义是「增加一个字段」，不是「设置字段」。
- 若 `add_header` 会覆盖，nginx 就没必要提供 `proxy_hide_header`：「The `proxy_hide_header` directive sets additional fields that will not be passed.」（[nginx docs: proxy_hide_header](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_hide_header)）

### 3.2 源码依据

`add_header` 的实现是往 `r->headers_out.headers` 里 **push 一个新元素**，不查找同名旧字段：

```c
static ngx_int_t
ngx_http_add_header(ngx_http_request_t *r, ngx_http_header_val_t *hv, ngx_str_t *value)
{
    ...
    h = ngx_list_push(&r->headers_out.headers);   /* 追加，不替换 */
    ...
}
```

（[nginx/src/http/modules/ngx_http_headers_filter_module.c](https://github.com/nginx/nginx/blob/master/src/http/modules/ngx_http_headers_filter_module.c)）

顺带一个实现细节：`if (value->len)` —— 值为空字符串时该头**不会**被添加，所以用变量/map 驱动的头在「清空配置」时自然消失。

### 3.3 实测（本仓库打包的 nginx 1.26.2）

上游 server 自带 `X-Frame-Options: DENY`，代理层 `add_header X-Frame-Options "SAMEORIGIN" always;`：

```
$ curl -s -D - -o /dev/null http://127.0.0.1:28090/
...
Strict-Transport-Security: max-age=111     ← 上游自己发的
X-Frame-Options: DENY                     ← 上游的，排在前面
Accept-Ranges: bytes
X-Content-Type-Options: nosniff           ← nginx 追加的
Referrer-Policy: strict-origin-when-cross-origin
X-Frame-Options: SAMEORIGIN               ← nginx 追加的，排在后面
```

即：**同名头两条都在，上游那条在前，`add_header` 那条在后**。

### 3.4 重复字段最终怎么被消费，取决于每个头自己的规范（这点决定了「追加」是良性还是有害）

| 头 | 出现多个字段时 | 后果 |
| --- | --- | --- |
| `Strict-Transport-Security` | UA 只处理**第一个**（[RFC 6797 §8.1](https://www.rfc-editor.org/rfc/rfc6797#section-8.1)） | 上游在前 → **上游的值赢**，代理层设了等于没设（实测见 3.3） |
| `X-Content-Type-Options` | Fetch 规范判 nosniff 时取 `values[0]`（[Fetch §3.6](https://fetch.spec.whatwg.org/#x-content-type-options-header)） | 只有 `nosniff` 一个合法值，重复实际上无害；上游在前时上游那条生效 |
| `Content-Security-Policy` | 每条策略**都生效**，等于取交集（[CSP3 §8.1](https://www.w3.org/TR/CSP3/#multiple-policies)、[MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy#multiple_content_security_policies)） | 更容易砸站：任何一条策略拒绝某资源都会被阻断 |
| `Referrer-Policy` | 多个来源或多值时取**最新/最后一个**有效值（[W3C Referrer Policy](https://www.w3.org/TR/referrer-policy/)） | 代理层追加在后的值会覆盖上游语义（方向与 HSTS 相反） |
| `X-Frame-Options` | RFC 7034 只说三个值互斥、「头字段必须被设置为恰好一个」，**未规定**收到多个字段时的行为（[RFC 7034 §2.1](https://www.rfc-editor.org/rfc/rfc7034#section-2.1)） | 浏览器实现不一，属未定项，见第 6 节 |

**所以「默认开一组头」在代理层必须配套一个决定：遇到上游同名头怎么办。**

- 保守方案（推荐给默认项）：只对 `X-Content-Type-Options`、`Referrer-Policy` 这类「重复无害/上游优先」的头直接 `add_header`；对确实需要生效的，加 `proxy_hide_header <字段>` 再 `add_header`。
- 覆盖配方（实测有效，nginx 1.26.2）：

```nginx
location / {
    proxy_pass http://upstream;
    proxy_hide_header X-Frame-Options;                               # 丢掉上游的
    add_header X-Frame-Options "SAMEORIGIN" always;                  # 只留自己的
    add_header Strict-Transport-Security "max-age=63072000" always;  # 未 hide → 变成两个
}
```

```
Strict-Transport-Security: max-age=111            ← 上游的（会被 UA 采用）
X-Frame-Options: SAMEORIGIN                       ← 唯一一条，覆盖成功
Strict-Transport-Security: max-age=63072000       ← 自己的，被 RFC 6797 忽略
```

### 3.5 `always` 参数干什么

- 不带 `always`：只在响应码为 `200, 201, 204, 206, 301, 302, 303, 304, 307, 308` 时添加。
- 带 `always`（1.7.5 起）：**不论响应码**都添加——即 `4xx`、`5xx` 也会带上。（[nginx docs](https://nginx.org/en/docs/http/ngx_http_headers_module.html#add_header)、OWASP 亦点明「without the `always` option, the header will only be sent for certain status codes」）

实测（1.26.2，静态 404）：

```
404 + add_header X-Plain "v";          → 响应里没有 X-Plain
404 + add_header X-Plain "v" always;   → X-Plain: v
```

对反向代理尤其重要：上游 500/502/404、以及 nginx 自己用 `error_page` 生成的错误页都属「非 2xx/3xx」，**安全头默认应该一律带 `always`**，否则最需要加固的错误页恰好没有头。

---

## 4. `add_header` 的继承陷阱

### 4.1 规则原文

> These directives are inherited from the previous configuration level **if and only if there are no `add_header` directives defined on the current level**.
>
> — [nginx docs: add_header](https://nginx.org/en/docs/http/ngx_http_headers_module.html#add_header)

也就是说：**子层只要自己写了一条 `add_header`，父层（http/server）的所有 `add_header` 就对它完全失效**——不是合并，是整体替换继承链。这是「加了安全头反而丢掉父级头」的根因。

OWASP 把这坑写进了 nginx 小节：「adding `add_header Cache-Control "no-cache";` inside a location block silently drops every security header set in the enclosing server block for that location」，并给出三种解法：在每个块里重复写；放进共享文件用 `include` 引入；或 nginx ≥ 1.29.3 用 `add_header_inherit merge;`。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）

版本门槛很关键：

- `add_header_inherit on|off|merge` 是 **1.29.3** 才出现的指令，默认 `on`（即旧的「有则不继承」模型）。（[nginx docs: add_header_inherit](https://nginx.org/en/docs/http/ngx_http_headers_module.html#add_header_inherit)）
- **本仓库打包的是 1.26.2**（`scripts/prepare-nginx.sh:36` 的 `NGINX_VERSION`），所以 `add_header_inherit merge` 在 Meridian 上不可用，只能靠「重复写 / include / 只在一层写」。

### 4.2 实测（1.26.2）

server 级：

```nginx
add_header X-Content-Type-Options "nosniff" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
location / { proxy_pass http://upstream; }              # 无 add_header → 继承，两条都在
location /loc { add_header X-Loc "yes" always; ... }    # 有 add_header → 父级两条全丢
```

```
=== /   ===  X-Content-Type-Options: nosniff / Referrer-Policy: strict-origin-when-cross-origin   ← 都在
=== /loc === X-Loc: yes                                                                          ← 只剩它
```

注意 `/loc` 那次 200 响应里，上游自带的 `Strict-Transport-Security` / `X-Frame-Options` **仍在**（上游头走的是直通通道，见 3.1），丢掉的只是 server 级的 `X-Content-Type-Options` 与 `Referrer-Policy`。这条区别很关键：**继承陷阱只影响配置里写的 `add_header`，不影响上游自己发的头**——所以「头忽然少了」往往只在某些 location 上出现。

### 4.3 对本项目的直接含义

Meridian 生成的 server 块本身就带多个 location（`location <prefix>` 每规则一个，另有 `location = /502.html`），且未来必然出现「某条规则单独覆盖某个头」的需求。可选做法（按稳健度排序）：

1. **只在一个层级集中写**：所有安全头写在生成配置的一个「头块」里，location 内一律不写 `add_header`；需要同层不同值时，在该层**重复全套**（用生成器保证一致），而不是只补一条。
2. **用 `include` 引入共享片段**：把安全头放进一个生成出来的 `.conf`，每个需要的层级 `include` 一次——本质与 1 等价，但少一处漂移。
3. 目标 nginx 升到 ≥ 1.29.3 后使用 `add_header_inherit merge;`（需要先升级 sidecar 版本，属于有风险的改动）。
4. 不要指望 `if in location` 里写 `add_header`：本仓库的 sidecar 编译时 `--without-http_rewrite_module`（见 5.1），`if` 不可用。

---

## 5. 与本仓库实现相关的约束（实测记录）

### 5.1 sidecar 是裁剪版 nginx 1.26.2，没有 rewrite 模块

```
$ src-tauri/binaries/nginx-aarch64-apple-darwin -v
nginx version: nginx/1.26.2
$ ... -V | grep without-http
--without-http_rewrite_module  --without-http_gzip_module  --without-http_fastcgi_module  ...
```

实测 `return 200 "up\n";` 直接报 `unknown directive "return"`。推论：`if (...) { ... }` 同样不可用，因此 `add_header` 文档里列出的 `if in location` 上下文在本产品里不可用；配置生成不得依赖 `if`。

### 5.2 TLS passthrough（stream 层）无法加任何 HTTP 响应头

`stream { }` 里只有 TCP 转发（`src-tauri/src/config_engine/stream_config.rs` 的 `proxy_pass <host>:<port>`），nginx 看不到 HTTP 报文。规则 `tls_mode = "passthrough"` 时**只能在 UI 上禁用安全头选项**，否则用户会以为开了。

### 5.3 `proxy_set_header` 不是响应头

现有「自定义头」(`rule.custom_headers`) 生成的是 `proxy_set_header <k> <v>;`——那是**发给上游的请求头**。安全响应头必须用 `add_header`，两者不要混在一个字段里，否则语义完全不同且不会报错。

### 5.4 `error_page 502 /502.html;` + `internal` 的组合要配 `always`

生成器已设置 `proxy_intercept_errors on; error_page 502 /502.html;`。`location = /502.html` 本身没有 `add_header`，会继承 server 级（OK）；但若头只写在代理的 location 里，错误页就没有头。统一加 `always` 可规避「错误响应无头」。

### 5.5 CSP / Permissions-Policy 的值必须带引号

值里有 `;`、`'`，不加引号会让 nginx 配置直接解析失败：

```
add_header Content-Security-Policy default-src 'self'; script-src 'self' always;
→ nginx: [emerg] invalid parameter "self" ... (配置测试失败)

add_header Content-Security-Policy "default-src 'self'; img-src 'self' data:" always;
add_header Permissions-Policy "geolocation=(), camera=(), microphone=()" always;
→ 配置测试通过；实测回包：
  Content-Security-Policy: default-src 'self'; img-src 'self' data:
  Permissions-Policy: geolocation=(), camera=(), microphone=()
```

生成器拼配置时需要：整段值用双引号包裹，并对值内可能出现的 `"`、`$`（`add_header` 支持变量插值）做处理。

### 5.6 HSTS 的放置位置

- 只能放 TLS 终止的 server 块（`listen <port> ssl`）；80 端口的 server 块发了也会被 UA 忽略（[RFC 6797 §8.1](https://www.rfc-editor.org/rfc/rfc6797#section-8.1)）。
- `server_name _`（未填 domain 时生成）与 IP 直连场景不要发 HSTS（MDN：IP 不能是 HSTS host；且 `_` 兜底块会命中任意 Host）。本机开发站点（含 `localhost`）同样建议不发。

---

## 6. 分歧与未定项（不要假装有唯一答案）

1. **`X-Frame-Options` 默认 `DENY` 还是 `SAMEORIGIN`**：OWASP 推荐 `DENY`（最严）；做「不可知用户站点」的反向代理默认值时，`SAMEORIGIN` 破坏面更小但强度更低。争议点不是规范，而是产品取向：**安全评分导向选 `DENY`，可用性导向选 `SAMEORIGIN`**。
2. **重复 `X-Frame-Options` 的处理**：RFC 7034 §2.1 只规定「三个值互斥、必须恰好设置一个」，未定义收到多个字段怎么办；浏览器实现历史上不一致（RFC 自己也说 SAMEORIGIN 的判定「implementations vary」）。因此**不要在有意覆盖时留下两条**，务必 `proxy_hide_header` 后再 `add_header`。
3. **HSTS `preload`**：OWASP 的示例把 `preload` 放在「Recommended」里；而 Chrome preload 列表官方站点说得很直白：「While HSTS is recommended, HSTS preloading is **not** recommended.」（[hstspreload.org](https://hstspreload.org/)）且 `preload` 有「PERMANENT CONSEQUENCES」（[OWASP HSTS Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Strict_Transport_Security_Cheat_Sheet.html)）。→ 产品默认值坚决不带 `preload`。
4. **`Permissions-Policy` 成熟度**：OWASP 当作常规推荐头，MDN 标注 non-Baseline/experimental、部分主流浏览器支持不全。→ 依「规范优先」的判断，适合做「可选」，不适合做默认。
5. **要不要显式写 `Referrer-Policy`**：值与浏览器默认相同，写了不改变行为（OWASP 主张显式写；也可以认为纯噪音）。→ 作为默认开成本极低，可按评分工具需要决定。
6. **`X-XSS-Protection`**：本调研范围外，但顺手记录：OWASP 明确「Do not set this header or explicitly turn it off」（`X-XSS-Protection: 0`），**不要**加进默认集。（[OWASP Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html)）

---

## 7. 参考文献

一手来源（按访问日期 2026-11-16）：

**nginx 官方**
1. `add_header` / `add_header_inherit` / `add_trailer`：<https://nginx.org/en/docs/http/ngx_http_headers_module.html>
2. `proxy_hide_header` / `proxy_set_header` / `proxy_intercept_errors`：<https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_hide_header>
3. 源码 `ngx_http_headers_filter_module.c`（`add_header` 为 `ngx_list_push` 追加）：<https://github.com/nginx/nginx/blob/master/src/http/modules/ngx_http_headers_filter_module.c>
4. 源码 `ngx_http_upstream.c` / `ngx_http_upstream_hide_headers_hash`（`proxy_hide_header` 同样「本层有则不继承父层」）：<https://github.com/nginx/nginx/blob/master/src/http/ngx_http_upstream.c>

**MDN**
5. `X-Content-Type-Options`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/X-Content-Type-Options>
6. `X-Frame-Options`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/X-Frame-Options>
7. `Referrer-Policy`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy>
8. `Permissions-Policy`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Permissions-Policy>
9. `Content-Security-Policy`（含「Multiple content security policies」）：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy>
10. CSP `frame-ancestors`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/frame-ancestors>
11. `Strict-Transport-Security`：<https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Strict-Transport-Security>

**OWASP**
12. HTTP Security Response Headers Cheat Sheet：<https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html>
13. HTTP Strict Transport Security Cheat Sheet：<https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Strict_Transport_Security_Cheat_Sheet.html>

**RFC / 规范**
14. RFC 7034 `X-Frame-Options`（§2.1）：<https://www.rfc-editor.org/rfc/rfc7034#section-2.1>
15. RFC 6797 HSTS（§8.1 UA 处理、§6.1 头字段语法、附录 B「全端口适用」）：<https://www.rfc-editor.org/rfc/rfc6797#section-8.1>
16. WHATWG Fetch §3.6 `X-Content-Type-Options`：<https://fetch.spec.whatwg.org/#x-content-type-options-header>
17. W3C CSP3 §8.1 The effect of multiple policies：<https://www.w3.org/TR/CSP3/#multiple-policies>
18. W3C Referrer Policy：<https://www.w3.org/TR/referrer-policy/>
19. HSTS Preload List（Chrome，官方提交站；ramp-up 与提交要求）：<https://hstspreload.org/>

**本仓库内证据（只读引用）**
20. `scripts/prepare-nginx.sh:36`（`NGINX_VERSION=1.26.2`）
21. `src-tauri/src/config_engine/http_config.rs`（server 块与 location 生成、`error_page 502`、自定义请求头）
22. `src-tauri/src/config_engine/main_config.rs`（http/stream 骨架与 include）
23. `src-tauri/src/config_engine/stream_config.rs`（stream 层 TCP 转发，无法加 HTTP 头）

实测环境：`src-tauri/binaries/nginx-aarch64-apple-darwin`（nginx/1.26.2），临时前缀 `/tmp/ngx*`，回环高位端口，未触碰仓库文件。
