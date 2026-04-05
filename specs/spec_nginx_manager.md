# Spec: Nginx Manager (引擎管理)

## Changelog
| Date | Change | Reason |
|------|--------|--------|
| 2026-04-04 | Initial spec | Phase 2d |
| 2026-04-04 | Fix: quote all paths in nginx config for dirs with spaces (Application Support) | Bug fix |
| 2026-04-04 | Fix: pass `-c` flag to stop/reload commands so nginx finds correct pid file | Bug fix |
| 2026-04-04 | Add `append_to_error_log()`: writes lifecycle events to error.log for UI visibility | Enhancement |
| 2026-04-05 | Fix: Windows `CREATE_NO_WINDOW` for all subprocess spawning (nginx, tasklist, powershell, taskkill) | Bug fix: console flashing |
| 2026-04-05 | Fix: `status()` skips `test_config()` when nginx.conf doesn't exist | Bug fix: first-launch |

## Feature Description

管理内嵌 Nginx 子进程的完整生命周期：启动、停止、热重载、配置验证。提供引擎状态查询供 UI 显示。

## Use Cases

- UC-001: 应用启动时自动启动 Nginx
- UC-002: 代理规则变更后热重载 Nginx
- UC-003: 用户手动启停 Nginx 引擎
- UC-004: 配置验证失败时保持现有进程不受影响
- UC-005: 首次运行初始化数据目录和默认配置

## Interface Definition

### Tauri Commands

#### `get_engine_status`
- **Input:** none
- **Response:** `{ status: "running" | "stopped" | "error", pid?: number, uptime_seconds?: number, error_message?: string }`

#### `start_engine`
- **Input:** none
- **Response:** `{ status: "running", pid: number }`
- **Errors:** `ENGINE_ALREADY_RUNNING`, `ENGINE_START_FAILED`

#### `stop_engine`
- **Input:** none
- **Response:** `{ status: "stopped" }`
- **Errors:** `ENGINE_NOT_RUNNING`

#### `restart_engine`
- **Input:** none
- **Response:** `{ status: "running", pid: number }`
- **Notes:** stop → generate configs → start

### Internal Functions

#### `test_config() -> Result<TestResult>`
- 执行 `nginx -t -c {config_path}`
- **TestResult:** `{ success: bool, output: String }`

#### `reload() -> Result<()>`
- 执行 `nginx -s reload`
- 仅在 `test_config` 成功后调用

#### `safe_reload() -> Result<()>`
- 完整的安全重载流程：
  1. `Config Engine.write_configs()`
  2. `test_config()`
  3. 成功 → `reload()`
  4. 失败 → `Config Engine.restore_previous_configs()` → 返回错误

#### `get_bundled_nginx_path() -> PathBuf`
- 根据平台返回内嵌 Nginx 的路径
- Windows: `{app_dir}/resources/nginx/nginx.exe`
- Linux/macOS: `{app_dir}/resources/nginx/nginx`

#### `init_first_run(data_dir: &Path) -> Result<()>`
- 创建目录结构：`nginx/conf.d/`, `nginx/stream.d/`, `nginx/certs/`, `nginx/logs/`
- 生成默认 `nginx.conf`
- 写入初始空配置

## Business Rules

1. **Nginx 路径解析顺序**：用户自定义路径（AppSettings `nginx_path`）→ 内嵌路径 → 系统 PATH
2. **启动前检查**：验证 nginx binary 存在且可执行
3. **进程监控**：通过 PID 文件或进程 handle 跟踪 Nginx 状态
4. **优雅停止**：使用 `nginx -s quit`（等待请求完成），超时 10s 后强制 kill
5. **热重载不中断服务**：`nginx -s reload` 保证 worker 进程平滑切换
6. **首次运行检测**：检查数据目录是否存在 `nginx/nginx.conf`，不存在则执行 `init_first_run`
7. **应用退出时停止 Nginx**：通过 Tauri exit hook（托盘退出菜单）确保 Nginx 进程一同退出
8. **配置验证输出**：将 `nginx -t` 的 stdout/stderr 完整传递给前端，便于用户排错
9. **路径引号处理**：生成的 nginx.conf 中所有文件路径（pid、error_log、access_log、include、ssl_certificate）使用双引号包裹，支持包含空格的目录（如 macOS `Application Support`）
10. **stop/reload 传递 -c**：`nginx -s quit` 和 `nginx -s reload` 必须传入 `-c` 指向自定义 config 路径，否则 nginx 使用默认路径查找 pid 文件导致失败
11. **生命周期日志**：nginx 启动成功/失败、停止成功/失败、重载成功/失败、配置测试失败均写入 error.log（带 `[meridian]` 标签和时间戳），便于用户在日志页面查看
12. **Windows 隐藏控制台窗口**：所有通过 `Command` 启动的外部进程（nginx、tasklist、powershell、taskkill、where）在 Windows 上设置 `CREATE_NO_WINDOW` (0x08000000) 标志，防止前端轮询 status 时终端窗口反复闪烁。通过 `nginx_command()` 辅助函数统一处理 nginx 进程创建
13. **状态查询容错**：`status()` 在 nginx 未运行且 `nginx.conf` 不存在时跳过 `test_config()` 调用，直接返回 "stopped" 状态。避免首次安装时配置文件未生成导致的报错循环

## Test Points

| TP-ID | Category | Input | Expected Output | Notes |
|-------|----------|-------|-----------------|-------|
| TP-001 | Normal | start_engine (nginx stopped) | Status = running, PID set | |
| TP-002 | Normal | stop_engine (nginx running) | Status = stopped | |
| TP-003 | Normal | restart_engine | Stop then start, new PID | |
| TP-004 | Error | start_engine (already running) | ENGINE_ALREADY_RUNNING error | |
| TP-005 | Error | stop_engine (not running) | ENGINE_NOT_RUNNING error | |
| TP-006 | Normal | test_config with valid config | success=true, output contains "syntax is ok" | |
| TP-007 | Error | test_config with invalid config | success=false, output contains error details | |
| TP-008 | Normal | safe_reload with valid config | Config written, nginx reloaded | |
| TP-009 | Error | safe_reload with invalid config | Previous config restored, nginx untouched | |
| TP-010 | Normal | get_engine_status (running) | status="running", pid present, uptime > 0 | |
| TP-011 | Normal | get_engine_status (stopped) | status="stopped", no pid | |
| TP-012 | Normal | init_first_run on empty data dir | All directories created, nginx.conf generated | |
| TP-013 | Boundary | init_first_run on existing data dir | No overwrite, no error | Idempotent |
| TP-014 | Normal | App exit → nginx process cleanup | Nginx process terminated | |
| TP-015 | Error | start_engine with missing nginx binary | ENGINE_START_FAILED with clear error message | |
| TP-016 | Normal (Windows) | status() polling every 5s | No console window flashes | CREATE_NO_WINDOW |
| TP-017 | Normal | status() when nginx.conf doesn't exist | Returns "stopped", no test_config() call | First-launch |

## Implementation Map

| Spec Item | Code File(s) | Function / Class | Notes |
|-----------|-------------|-----------------|-------|
| Start/Stop/Reload/Status | `src-tauri/src/nginx_manager/mod.rs` | `start()`, `stop()`, `reload()`, `status()` | All pass `-c` and `-p` flags; all use `nginx_command()` on Windows |
| Windows console suppression | `src-tauri/src/nginx_manager/mod.rs` | `nginx_command()` | Sets `CREATE_NO_WINDOW` on Windows |
| Config test | `src-tauri/src/nginx_manager/mod.rs` | `test_config()` | |
| Nginx path discovery | `src-tauri/src/nginx_manager/mod.rs` | `get_bundled_nginx_path()` | Candidate list + PATH fallback |
| Lifecycle logging | `src-tauri/src/nginx_manager/mod.rs` | `append_to_error_log()` | Writes to nginx error.log |
| Process uptime | `src-tauri/src/nginx_manager/mod.rs` | `get_process_uptime()`, `parse_etime()` | Uses `ps -o etime` |
| Path quoting | `src-tauri/src/config_engine/main_config.rs` | `generate_main_config()` | All directives quoted |
| Path quoting (HTTP) | `src-tauri/src/config_engine/http_config.rs` | `generate_server_block()` | SSL cert paths quoted |
| Path quoting (Stream) | `src-tauri/src/config_engine/stream_config.rs` | `generate_stream_block()` | SSL cert paths quoted |
