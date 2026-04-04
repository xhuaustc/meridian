# Hosts Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add local hosts file management to Meridian, with an independent management page, system hosts file writing via managed block, and smart prompts in the proxy form.

**Architecture:** New `hosts_manager` Rust module handles reading/writing the system hosts file with platform-specific elevation. A `hosts_repo` SQLite repo stores entries as source of truth. Frontend follows the existing AccessPage pattern: Zustand store, API layer, page component, and Dialog-based CRUD. Proxy form integration uses post-save check + Dialog prompt.

**Tech Stack:** Rust (rusqlite, uuid, chrono, std::process::Command for elevation), React + TypeScript, Zustand, Tauri IPC, i18next

---

### Task 1: Rust Data Model & Migration

**Files:**
- Modify: `src-tauri/src/store/models.rs`
- Modify: `src-tauri/src/store/mod.rs`

- [ ] **Step 1: Add HostEntry and CreateHostEntry structs to models.rs**

Add after the `AccessListDetail` struct (around line 199):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub id: String,
    pub ip: String,
    pub hostname: String,
    pub comment: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHostEntry {
    pub ip: String,
    pub hostname: String,
    pub comment: Option<String>,
}
```

- [ ] **Step 2: Add host_entries table migration to mod.rs**

In `run_migrations()`, add inside the `execute_batch` string, after the `acme_accounts` table:

```sql
CREATE TABLE IF NOT EXISTS host_entries (
    id TEXT PRIMARY KEY,
    ip TEXT NOT NULL,
    hostname TEXT NOT NULL UNIQUE,
    comment TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/store/models.rs src-tauri/src/store/mod.rs
git commit -m "feat(hosts): add HostEntry model and database migration"
```

---

### Task 2: Hosts Repository (SQLite CRUD)

**Files:**
- Create: `src-tauri/src/store/hosts_repo.rs`
- Modify: `src-tauri/src/store/mod.rs` (add `pub mod hosts_repo;`)

- [ ] **Step 1: Register the module**

In `src-tauri/src/store/mod.rs`, add after `pub mod dns_credential_repo;`:

```rust
pub mod hosts_repo;
```

- [ ] **Step 2: Create hosts_repo.rs with full CRUD**

Create `src-tauri/src/store/hosts_repo.rs`:

```rust
use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::store::models::{CreateHostEntry, HostEntry};

fn row_to_host_entry(row: &rusqlite::Row) -> rusqlite::Result<HostEntry> {
    Ok(HostEntry {
        id: row.get("id")?,
        ip: row.get("ip")?,
        hostname: row.get("hostname")?,
        comment: row.get("comment")?,
        enabled: row.get("enabled")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_all(conn: &Connection, keyword: Option<&str>) -> Result<Vec<HostEntry>, AppError> {
    let entries = if let Some(kw) = keyword {
        let pattern = format!("%{}%", kw);
        let mut stmt = conn.prepare(
            "SELECT * FROM host_entries WHERE hostname LIKE ?1 OR ip LIKE ?1 OR comment LIKE ?1 ORDER BY hostname ASC",
        )?;
        stmt.query_map(params![pattern], |row| row_to_host_entry(row))?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = conn.prepare("SELECT * FROM host_entries ORDER BY hostname ASC")?;
        stmt.query_map([], |row| row_to_host_entry(row))?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(entries)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<HostEntry, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM host_entries WHERE id = ?1")?;
    let entry = stmt
        .query_row(params![id], |row| row_to_host_entry(row))
        .map_err(|_| AppError::NotFound(format!("Host entry '{}' not found", id)))?;
    Ok(entry)
}

pub fn find_by_hostname(conn: &Connection, hostname: &str) -> Result<Option<HostEntry>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM host_entries WHERE LOWER(hostname) = LOWER(?1)")?;
    let result = stmt
        .query_row(params![hostname], |row| row_to_host_entry(row))
        .ok();
    Ok(result)
}

pub fn create(conn: &Connection, input: &CreateHostEntry) -> Result<HostEntry, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO host_entries (id, ip, hostname, comment, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
        params![id, input.ip, input.hostname, input.comment, now],
    )?;

    get_by_id(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    ip: Option<&str>,
    hostname: Option<&str>,
    comment: Option<&str>,
) -> Result<HostEntry, AppError> {
    let existing = get_by_id(conn, id)?;
    let ip = ip.unwrap_or(&existing.ip);
    let hostname = hostname.unwrap_or(&existing.hostname);
    let comment = comment.or(existing.comment.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE host_entries SET ip = ?1, hostname = ?2, comment = ?3, updated_at = ?4 WHERE id = ?5",
        params![ip, hostname, comment, now, id],
    )?;

    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM host_entries WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Host entry '{}' not found", id)));
    }
    Ok(())
}

pub fn toggle(conn: &Connection, id: &str, enabled: bool) -> Result<HostEntry, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE host_entries SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        params![enabled, now, id],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("Host entry '{}' not found", id)));
    }
    get_by_id(conn, id)
}

pub fn list_enabled(conn: &Connection) -> Result<Vec<HostEntry>, AppError> {
    let mut stmt = conn.prepare("SELECT * FROM host_entries WHERE enabled = 1 ORDER BY hostname ASC")?;
    let entries = stmt
        .query_map([], |row| row_to_host_entry(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/store/hosts_repo.rs src-tauri/src/store/mod.rs
git commit -m "feat(hosts): add hosts_repo SQLite CRUD operations"
```

---

### Task 3: Input Validation

**Files:**
- Modify: `src-tauri/src/validators.rs`

- [ ] **Step 1: Add validate_host_entry function**

Add at the end of `validators.rs`:

```rust
pub fn validate_host_entry(ip: &str, hostname: &str) -> Result<(), AppError> {
    // Validate IP
    let ip = ip.trim();
    if ip.is_empty() {
        return Err(AppError::Validation("IP address must not be empty".to_string()));
    }
    if ip.parse::<std::net::Ipv4Addr>().is_err() && ip.parse::<std::net::Ipv6Addr>().is_err() {
        return Err(AppError::Validation(format!("Invalid IP address '{}'", ip)));
    }

    // Validate hostname
    let hostname = hostname.trim();
    if hostname.is_empty() {
        return Err(AppError::Validation("Hostname must not be empty".to_string()));
    }
    if hostname.len() > 253 {
        return Err(AppError::Validation("Hostname must not exceed 253 characters".to_string()));
    }
    // Basic hostname format: alphanumeric, hyphens, dots
    let valid = hostname.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    if !valid {
        return Err(AppError::Validation(format!("Invalid hostname format '{}'", hostname)));
    }

    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/validators.rs
git commit -m "feat(hosts): add hostname and IP validation"
```

---

### Task 4: Hosts Manager (System Hosts File Read/Write)

**Files:**
- Create: `src-tauri/src/hosts_manager.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod hosts_manager;`)

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add after `mod error;`:

```rust
mod hosts_manager;
```

- [ ] **Step 2: Create hosts_manager.rs**

Create `src-tauri/src/hosts_manager.rs`:

```rust
use std::path::Path;

use tracing::{info, warn};

use crate::error::AppError;
use crate::store::models::HostEntry;

const BLOCK_START: &str = "# >>> Meridian managed — DO NOT EDIT THIS BLOCK";
const BLOCK_END: &str = "# <<< Meridian managed";

/// Get the system hosts file path for the current platform.
pub fn hosts_file_path() -> &'static str {
    if cfg!(target_os = "windows") {
        r"C:\Windows\System32\drivers\etc\hosts"
    } else {
        "/etc/hosts"
    }
}

/// Generate the Meridian managed block content from enabled entries.
pub fn generate_block(entries: &[HostEntry]) -> String {
    let mut lines = vec![BLOCK_START.to_string()];
    for entry in entries {
        let line = if let Some(ref comment) = entry.comment {
            format!("{}\t{}\t# {}", entry.ip, entry.hostname, comment)
        } else {
            format!("{}\t{}", entry.ip, entry.hostname)
        };
        lines.push(line);
    }
    lines.push(BLOCK_END.to_string());
    lines.join("\n")
}

/// Replace or append the Meridian managed block in the hosts file content.
pub fn replace_block(hosts_content: &str, new_block: &str) -> String {
    if let (Some(start_pos), Some(end_pos)) = (
        hosts_content.find(BLOCK_START),
        hosts_content.find(BLOCK_END),
    ) {
        let end_pos = end_pos + BLOCK_END.len();
        // Consume trailing newline if present
        let end_pos = if hosts_content[end_pos..].starts_with('\n') {
            end_pos + 1
        } else {
            end_pos
        };
        let mut result = String::new();
        result.push_str(&hosts_content[..start_pos]);
        result.push_str(new_block);
        result.push('\n');
        result.push_str(&hosts_content[end_pos..]);
        result
    } else {
        // No existing block, append
        let mut result = hosts_content.to_string();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push('\n');
        result.push_str(new_block);
        result.push('\n');
        result
    }
}

/// Write content to the hosts file with platform-specific elevation.
pub fn write_hosts_elevated(content: &str) -> Result<(), AppError> {
    let path = hosts_file_path();
    info!("Writing hosts file with elevation: {}", path);

    if cfg!(target_os = "macos") {
        write_hosts_macos(content, path)
    } else if cfg!(target_os = "windows") {
        write_hosts_windows(content, path)
    } else {
        write_hosts_linux(content, path)
    }
}

fn write_hosts_macos(content: &str, path: &str) -> Result<(), AppError> {
    // Use osascript to run with admin privileges
    let script = format!(
        r#"do shell script "cat > '{}'" with administrator privileges"#,
        path
    );
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(|e| AppError::Io(e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("-128") {
            return Err(AppError::Validation("Permission denied: user cancelled authentication".to_string()));
        }
        return Err(AppError::Config(format!("Failed to write hosts file: {}", stderr)));
    }

    info!("Hosts file updated successfully");
    Ok(())
}

fn write_hosts_linux(content: &str, path: &str) -> Result<(), AppError> {
    let output = std::process::Command::new("pkexec")
        .arg("tee")
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(|e| AppError::Io(e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("dismissed") || stderr.contains("Not authorized") {
            return Err(AppError::Validation("Permission denied: user cancelled authentication".to_string()));
        }
        return Err(AppError::Config(format!("Failed to write hosts file: {}", stderr)));
    }

    info!("Hosts file updated successfully");
    Ok(())
}

fn write_hosts_windows(content: &str, path: &str) -> Result<(), AppError> {
    // On Windows, try direct write first (works if running as admin)
    match std::fs::write(path, content) {
        Ok(_) => {
            info!("Hosts file updated successfully (direct write)");
            Ok(())
        }
        Err(e) => {
            warn!("Direct write failed ({}), need administrator privileges", e);
            Err(AppError::Validation(
                "Permission denied: please run Meridian as administrator to manage hosts".to_string(),
            ))
        }
    }
}

/// Read the current hosts file content.
pub fn read_hosts_file() -> Result<String, AppError> {
    let path = hosts_file_path();
    std::fs::read_to_string(path).map_err(|e| {
        AppError::Config(format!("Failed to read hosts file '{}': {}", path, e))
    })
}

/// Sync enabled host entries to the system hosts file.
pub fn sync_to_system(entries: &[HostEntry]) -> Result<(), AppError> {
    let current = read_hosts_file()?;
    let block = generate_block(entries);
    let new_content = replace_block(&current, &block);
    write_hosts_elevated(&new_content)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/hosts_manager.rs src-tauri/src/lib.rs
git commit -m "feat(hosts): add hosts_manager for system hosts file read/write with elevation"
```

---

### Task 5: Hosts Manager Unit Tests

**Files:**
- Modify: `src-tauri/src/hosts_manager.rs` (add `#[cfg(test)]` module)

- [ ] **Step 1: Add tests at the bottom of hosts_manager.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::models::HostEntry;

    fn make_entry(ip: &str, hostname: &str, comment: Option<&str>) -> HostEntry {
        HostEntry {
            id: "test-id".to_string(),
            ip: ip.to_string(),
            hostname: hostname.to_string(),
            comment: comment.map(|s| s.to_string()),
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_generate_block_empty() {
        let block = generate_block(&[]);
        assert_eq!(block, format!("{}\n{}", BLOCK_START, BLOCK_END));
    }

    #[test]
    fn test_generate_block_with_entries() {
        let entries = vec![
            make_entry("127.0.0.1", "app.local", Some("frontend")),
            make_entry("192.168.1.100", "api.dev.com", None),
        ];
        let block = generate_block(&entries);
        assert!(block.contains("127.0.0.1\tapp.local\t# frontend"));
        assert!(block.contains("192.168.1.100\tapi.dev.com"));
        assert!(block.starts_with(BLOCK_START));
        assert!(block.ends_with(BLOCK_END));
    }

    #[test]
    fn test_replace_block_no_existing() {
        let hosts = "127.0.0.1 localhost\n::1 localhost\n";
        let block = generate_block(&[make_entry("10.0.0.1", "test.local", None)]);
        let result = replace_block(hosts, &block);
        assert!(result.contains("127.0.0.1 localhost"));
        assert!(result.contains("::1 localhost"));
        assert!(result.contains(BLOCK_START));
        assert!(result.contains("10.0.0.1\ttest.local"));
        assert!(result.contains(BLOCK_END));
    }

    #[test]
    fn test_replace_block_existing() {
        let hosts = format!(
            "127.0.0.1 localhost\n{}\n1.2.3.4\told.entry\n{}\n::1 localhost\n",
            BLOCK_START, BLOCK_END
        );
        let block = generate_block(&[make_entry("10.0.0.1", "new.entry", None)]);
        let result = replace_block(&hosts, &block);
        assert!(result.contains("127.0.0.1 localhost"));
        assert!(result.contains("::1 localhost"));
        assert!(result.contains("10.0.0.1\tnew.entry"));
        assert!(!result.contains("old.entry"));
    }

    #[test]
    fn test_replace_block_preserves_surrounding() {
        let hosts = format!(
            "# custom\n127.0.0.1 localhost\n{}\nold\n{}\n# footer\n",
            BLOCK_START, BLOCK_END
        );
        let block = generate_block(&[]);
        let result = replace_block(&hosts, &block);
        assert!(result.contains("# custom"));
        assert!(result.contains("127.0.0.1 localhost"));
        assert!(result.contains("# footer"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo test hosts_manager --lib 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/hosts_manager.rs
git commit -m "test(hosts): add unit tests for hosts file block generation and replacement"
```

---

### Task 6: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/hosts.rs`
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod hosts;`)
- Modify: `src-tauri/src/lib.rs` (register commands in `generate_handler![]`)

- [ ] **Step 1: Register the commands module**

In `src-tauri/src/commands/mod.rs`, add after `pub mod engine;`:

```rust
pub mod hosts;
```

- [ ] **Step 2: Create commands/hosts.rs**

Create `src-tauri/src/commands/hosts.rs`:

```rust
use tauri::State;

use crate::error::AppError;
use crate::hosts_manager;
use crate::store::hosts_repo;
use crate::store::models::{CreateHostEntry, HostEntry};
use crate::validators;
use crate::AppState;

#[tauri::command]
pub async fn list_hosts(
    keyword: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HostEntry>, AppError> {
    let db = state.lock_db()?;
    hosts_repo::list_all(&db, keyword.as_deref())
}

#[tauri::command]
pub async fn create_host(
    input: CreateHostEntry,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    validators::validate_host_entry(&input.ip, &input.hostname)?;

    let entry = {
        let db = state.lock_db()?;

        // Check hostname uniqueness
        if let Some(existing) = hosts_repo::find_by_hostname(&db, &input.hostname)? {
            return Err(AppError::Conflict(format!(
                "Hostname '{}' already exists (id: {})",
                existing.hostname, existing.id
            )));
        }

        hosts_repo::create(&db, &input)?
    };

    // Sync to system hosts file (best-effort)
    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
        // Don't fail the command — DB is source of truth
    }

    Ok(entry)
}

#[tauri::command]
pub async fn update_host(
    id: String,
    ip: Option<String>,
    hostname: Option<String>,
    comment: Option<String>,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    if let Some(ref ip) = ip {
        if let Some(ref hn) = hostname {
            validators::validate_host_entry(ip, hn)?;
        } else {
            // Validate just IP by using a dummy hostname
            validators::validate_host_entry(ip, "dummy.local")?;
        }
    }
    if let Some(ref hn) = hostname {
        if ip.is_none() {
            // Validate just hostname by using a dummy IP
            validators::validate_host_entry("127.0.0.1", hn)?;
        }
    }

    let entry = {
        let db = state.lock_db()?;

        // Check hostname uniqueness if changing
        if let Some(ref hn) = hostname {
            if let Some(existing) = hosts_repo::find_by_hostname(&db, hn)? {
                if existing.id != id {
                    return Err(AppError::Conflict(format!(
                        "Hostname '{}' already exists (id: {})",
                        existing.hostname, existing.id
                    )));
                }
            }
        }

        hosts_repo::update(&db, &id, ip.as_deref(), hostname.as_deref(), comment.as_deref())?
    };

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(entry)
}

#[tauri::command]
pub async fn delete_host(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    {
        let db = state.lock_db()?;
        hosts_repo::delete(&db, &id)?;
    }

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_host(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<HostEntry, AppError> {
    let entry = {
        let db = state.lock_db()?;
        hosts_repo::toggle(&db, &id, enabled)?
    };

    if let Err(e) = sync_hosts_to_system(&state) {
        tracing::warn!("Failed to sync hosts file: {}", e);
    }

    Ok(entry)
}

#[tauri::command]
pub async fn check_hostname_exists(
    hostname: String,
    exclude_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<HostEntry>, AppError> {
    let db = state.lock_db()?;
    let found = hosts_repo::find_by_hostname(&db, &hostname)?;
    // Exclude self if editing
    if let Some(ref entry) = found {
        if let Some(ref eid) = exclude_id {
            if &entry.id == eid {
                return Ok(None);
            }
        }
    }
    Ok(found)
}

#[tauri::command]
pub async fn sync_hosts_file(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    sync_hosts_to_system(&state)
}

/// Helper: read all enabled entries from DB and sync to system hosts file.
fn sync_hosts_to_system(state: &AppState) -> Result<(), AppError> {
    let db = state.lock_db()?;
    let entries = hosts_repo::list_enabled(&db)?;
    drop(db);
    hosts_manager::sync_to_system(&entries)
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, inside `generate_handler![]`, add after the access list commands block:

```rust
// Host management commands
commands::hosts::list_hosts,
commands::hosts::create_host,
commands::hosts::update_host,
commands::hosts::delete_host,
commands::hosts::toggle_host,
commands::hosts::check_hostname_exists,
commands::hosts::sync_hosts_file,
```

- [ ] **Step 4: Verify it compiles**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/hosts.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(hosts): add Tauri IPC commands for hosts management"
```

---

### Task 7: TypeScript Types & API Layer

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add HostEntry type to types/index.ts**

Add at the end of `src/types/index.ts`:

```typescript
export interface HostEntry {
  id: string;
  ip: string;
  hostname: string;
  comment: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateHostEntry {
  ip: string;
  hostname: string;
  comment?: string | null;
}
```

- [ ] **Step 2: Add API functions to api.ts**

Add at the end of `src/lib/api.ts` (before the `// --- Tray ---` section):

```typescript
// --- Hosts ---
export const listHosts = (keyword?: string) =>
  invoke<HostEntry[]>('list_hosts', { keyword });

export const createHost = (input: CreateHostEntry) =>
  invoke<HostEntry>('create_host', { input });

export const updateHost = (
  id: string,
  ip?: string,
  hostname?: string,
  comment?: string,
) => invoke<HostEntry>('update_host', { id, ip, hostname, comment });

export const deleteHost = (id: string) =>
  invoke<void>('delete_host', { id });

export const toggleHost = (id: string, enabled: boolean) =>
  invoke<HostEntry>('toggle_host', { id, enabled });

export const checkHostnameExists = (hostname: string, excludeId?: string) =>
  invoke<HostEntry | null>('check_hostname_exists', { hostname, excludeId });

export const syncHostsFile = () =>
  invoke<void>('sync_hosts_file');
```

Also add `HostEntry, CreateHostEntry` to the import list from `'../types'` at the top of api.ts.

- [ ] **Step 3: Commit**

```bash
git add src/types/index.ts src/lib/api.ts
git commit -m "feat(hosts): add TypeScript types and API layer for hosts management"
```

---

### Task 8: Zustand Store

**Files:**
- Create: `src/stores/hosts-store.ts`

- [ ] **Step 1: Create hosts-store.ts**

Create `src/stores/hosts-store.ts`:

```typescript
import { create } from 'zustand';
import type { HostEntry } from '../types';
import * as api from '../lib/api';

interface HostsStore {
  entries: HostEntry[];
  loading: boolean;
  error: string | null;
  fetchEntries: (keyword?: string) => Promise<void>;
  createEntry: (ip: string, hostname: string, comment?: string) => Promise<HostEntry>;
  updateEntry: (id: string, ip?: string, hostname?: string, comment?: string) => Promise<HostEntry>;
  deleteEntry: (id: string) => Promise<void>;
  toggleEntry: (id: string, enabled: boolean) => Promise<HostEntry>;
  syncToSystem: () => Promise<void>;
}

export const useHostsStore = create<HostsStore>((set, get) => ({
  entries: [],
  loading: false,
  error: null,
  fetchEntries: async (keyword?: string) => {
    set({ loading: true, error: null });
    try {
      const entries = await api.listHosts(keyword);
      set({ entries, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
  createEntry: async (ip, hostname, comment) => {
    const entry = await api.createHost({ ip, hostname, comment });
    await get().fetchEntries();
    return entry;
  },
  updateEntry: async (id, ip, hostname, comment) => {
    const entry = await api.updateHost(id, ip, hostname, comment);
    set((state) => ({
      entries: state.entries.map((e) => (e.id === id ? entry : e)),
    }));
    return entry;
  },
  deleteEntry: async (id) => {
    await api.deleteHost(id);
    set((state) => ({
      entries: state.entries.filter((e) => e.id !== id),
    }));
  },
  toggleEntry: async (id, enabled) => {
    const entry = await api.toggleHost(id, enabled);
    set((state) => ({
      entries: state.entries.map((e) => (e.id === id ? entry : e)),
    }));
    return entry;
  },
  syncToSystem: async () => {
    await api.syncHostsFile();
  },
}));
```

- [ ] **Step 2: Commit**

```bash
git add src/stores/hosts-store.ts
git commit -m "feat(hosts): add Zustand store for hosts management"
```

---

### Task 9: i18n Translations

**Files:**
- Modify: `src/locales/en/common.json`
- Modify: `src/locales/zh/common.json`

- [ ] **Step 1: Add English translations**

In `src/locales/en/common.json`, add to the `"nav"` section:

```json
"hosts": "Hosts"
```

Add a new `"hosts"` section (after the `"access"` section):

```json
"hosts": {
  "title": "Hosts Management",
  "addEntry": "Add Entry",
  "hostname": "Hostname",
  "ip": "IP Address",
  "comment": "Comment",
  "hostnamePlaceholder": "e.g. app.local",
  "ipPlaceholder": "e.g. 127.0.0.1",
  "commentPlaceholder": "e.g. Frontend dev server",
  "createTitle": "Add Host Entry",
  "editTitle": "Edit Host Entry",
  "create": "Add",
  "save": "Save",
  "createSuccess": "Host entry added",
  "updateSuccess": "Host entry updated",
  "deleteConfirm": "Delete host entry \"{{hostname}}\"?",
  "deleteSuccess": "Host entry deleted",
  "toggleSuccess": "Host entry updated",
  "syncSuccess": "Hosts file synced to system",
  "syncButton": "Sync to System",
  "syncHint": "Force write all entries to system hosts file",
  "emptyTitle": "No host entries",
  "emptyDesc": "Add host entries to map domain names to IP addresses",
  "searchPlaceholder": "Search by hostname, IP, or comment...",
  "promptTitle": "Add Hosts Entry?",
  "promptMessage": "Domain \"{{domain}}\" has no local hosts entry. Add one?",
  "promptAdd": "Add",
  "promptSkip": "Skip",
  "deletePromptTitle": "Clean Up Hosts Entry?",
  "deletePromptMessage": "Domain \"{{domain}}\" is no longer used by any proxy rule. Delete its hosts entry?",
  "deletePromptConfirm": "Delete Entry",
  "deletePromptKeep": "Keep",
  "syncFailed": "Hosts file not updated. You can sync later from the Hosts page."
}
```

- [ ] **Step 2: Add Chinese translations**

In `src/locales/zh/common.json`, add to the `"nav"` section:

```json
"hosts": "域名解析"
```

Add a new `"hosts"` section:

```json
"hosts": {
  "title": "域名解析管理",
  "addEntry": "添加条目",
  "hostname": "域名",
  "ip": "IP 地址",
  "comment": "备注",
  "hostnamePlaceholder": "如 app.local",
  "ipPlaceholder": "如 127.0.0.1",
  "commentPlaceholder": "如 前端开发服务",
  "createTitle": "添加域名解析",
  "editTitle": "编辑域名解析",
  "create": "添加",
  "save": "保存",
  "createSuccess": "域名解析已添加",
  "updateSuccess": "域名解析已更新",
  "deleteConfirm": "删除域名解析 \"{{hostname}}\"？",
  "deleteSuccess": "域名解析已删除",
  "toggleSuccess": "域名解析已更新",
  "syncSuccess": "已同步到系统 hosts 文件",
  "syncButton": "同步到系统",
  "syncHint": "强制将所有条目写入系统 hosts 文件",
  "emptyTitle": "暂无域名解析",
  "emptyDesc": "添加域名解析条目，将域名映射到 IP 地址",
  "searchPlaceholder": "搜索域名、IP 或备注...",
  "promptTitle": "添加域名解析？",
  "promptMessage": "域名 \"{{domain}}\" 尚未配置本地解析，是否添加？",
  "promptAdd": "添加",
  "promptSkip": "跳过",
  "deletePromptTitle": "清理域名解析？",
  "deletePromptMessage": "域名 \"{{domain}}\" 已无代理规则使用，是否删除对应的 hosts 条目？",
  "deletePromptConfirm": "删除条目",
  "deletePromptKeep": "保留",
  "syncFailed": "hosts 文件未更新，可稍后在域名解析页面点击同步"
}
```

- [ ] **Step 3: Commit**

```bash
git add src/locales/en/common.json src/locales/zh/common.json
git commit -m "feat(hosts): add i18n translations for hosts management"
```

---

### Task 10: Hosts Page Component

**Files:**
- Create: `src/pages/HostsPage.tsx`

- [ ] **Step 1: Create HostsPage.tsx**

Create `src/pages/HostsPage.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, Pencil, Globe, RefreshCw } from 'lucide-react';
import { ContentToolbar } from '../components/layout/ContentToolbar';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { Toggle } from '../components/ui/Toggle';
import { Dialog, ConfirmDialog } from '../components/ui/Dialog';
import { useHostsStore } from '../stores/hosts-store';
import { useToastStore } from '../stores/toast-store';
import type { HostEntry } from '../types';

export function HostsPage() {
  const { t } = useTranslation('common');
  const { entries, fetchEntries, createEntry, updateEntry, deleteEntry, toggleEntry, syncToSystem } =
    useHostsStore();
  const addToast = useToastStore((s) => s.addToast);

  const [search, setSearch] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [editTarget, setEditTarget] = useState<HostEntry | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<HostEntry | null>(null);
  const [syncing, setSyncing] = useState(false);

  // Form state (shared by create and edit dialogs)
  const [formHostname, setFormHostname] = useState('');
  const [formIp, setFormIp] = useState('');
  const [formComment, setFormComment] = useState('');

  useEffect(() => {
    fetchEntries();
  }, [fetchEntries]);

  const filteredEntries = search
    ? entries.filter(
        (e) =>
          e.hostname.toLowerCase().includes(search.toLowerCase()) ||
          e.ip.includes(search) ||
          (e.comment && e.comment.toLowerCase().includes(search.toLowerCase())),
      )
    : entries;

  const openCreate = () => {
    setFormHostname('');
    setFormIp('127.0.0.1');
    setFormComment('');
    setShowCreate(true);
  };

  const openEdit = (entry: HostEntry) => {
    setFormHostname(entry.hostname);
    setFormIp(entry.ip);
    setFormComment(entry.comment ?? '');
    setEditTarget(entry);
  };

  const handleCreate = async () => {
    if (!formHostname.trim() || !formIp.trim()) return;
    try {
      await createEntry(formIp.trim(), formHostname.trim(), formComment.trim() || undefined);
      addToast('success', t('hosts.createSuccess'));
      setShowCreate(false);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleUpdate = async () => {
    if (!editTarget || !formHostname.trim() || !formIp.trim()) return;
    try {
      await updateEntry(
        editTarget.id,
        formIp.trim(),
        formHostname.trim(),
        formComment.trim() || undefined,
      );
      addToast('success', t('hosts.updateSuccess'));
      setEditTarget(null);
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteEntry(deleteTarget.id);
      addToast('success', t('hosts.deleteSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
    setDeleteTarget(null);
  };

  const handleToggle = async (entry: HostEntry) => {
    try {
      await toggleEntry(entry.id, !entry.enabled);
      addToast('success', t('hosts.toggleSuccess'));
    } catch (e) {
      addToast('error', String(e));
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      await syncToSystem();
      addToast('success', t('hosts.syncSuccess'));
    } catch (e) {
      addToast('error', String(e));
    } finally {
      setSyncing(false);
    }
  };

  const formDialog = (
    open: boolean,
    onClose: () => void,
    title: string,
    onSubmit: () => void,
    submitLabel: string,
  ) => (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button onClick={onClose}>{t('common.cancel')}</Button>
          <Button variant="primary" onClick={onSubmit}>
            {submitLabel}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.hostname')}
          </label>
          <Input
            value={formHostname}
            onChange={(e) => setFormHostname(e.target.value)}
            placeholder={t('hosts.hostnamePlaceholder')}
          />
        </div>
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.ip')}
          </label>
          <Input
            value={formIp}
            onChange={(e) => setFormIp(e.target.value)}
            placeholder={t('hosts.ipPlaceholder')}
          />
        </div>
        <div>
          <label className="block text-[12px] font-medium text-text-secondary mb-1">
            {t('hosts.comment')}
          </label>
          <Input
            value={formComment}
            onChange={(e) => setFormComment(e.target.value)}
            placeholder={t('hosts.commentPlaceholder')}
          />
        </div>
      </div>
    </Dialog>
  );

  return (
    <>
      <ContentToolbar title={t('hosts.title')}>
        <Button variant="primary" onClick={openCreate}>
          <Plus className="w-3.5 h-3.5" />
          {t('hosts.addEntry')}
        </Button>
      </ContentToolbar>
      <div className="p-6 overflow-y-auto flex-1">
        {/* Search */}
        <div className="mb-4">
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t('hosts.searchPlaceholder')}
            className="max-w-sm"
          />
        </div>

        {filteredEntries.length === 0 ? (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] py-16 flex flex-col items-center justify-center">
            <Globe className="w-10 h-10 text-text-tertiary mb-3" />
            <p className="text-[13px] font-medium text-text-secondary">
              {t('hosts.emptyTitle')}
            </p>
            <p className="text-[12px] text-text-tertiary mt-1">
              {t('hosts.emptyDesc')}
            </p>
          </div>
        ) : (
          <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-border text-text-tertiary text-[11px] uppercase tracking-wide">
                  <th className="px-4 py-2 text-left w-12" />
                  <th className="px-4 py-2 text-left">{t('hosts.hostname')}</th>
                  <th className="px-4 py-2 text-left">{t('hosts.ip')}</th>
                  <th className="px-4 py-2 text-left">{t('hosts.comment')}</th>
                  <th className="px-4 py-2 text-right w-24" />
                </tr>
              </thead>
              <tbody>
                {filteredEntries.map((entry) => (
                  <tr
                    key={entry.id}
                    className="border-b border-border last:border-b-0 hover:bg-bg-hover"
                  >
                    <td className="px-4 py-2">
                      <Toggle
                        checked={entry.enabled}
                        onChange={() => handleToggle(entry)}
                      />
                    </td>
                    <td className="px-4 py-2 font-mono">{entry.hostname}</td>
                    <td className="px-4 py-2 font-mono text-text-secondary">{entry.ip}</td>
                    <td className="px-4 py-2 text-text-tertiary">{entry.comment ?? '-'}</td>
                    <td className="px-4 py-2 text-right">
                      <div className="flex items-center justify-end gap-1">
                        <button
                          onClick={() => openEdit(entry)}
                          className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-text-primary"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => setDeleteTarget(entry)}
                          className="p-1 rounded hover:bg-bg-hover text-text-tertiary hover:text-error"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Sync button */}
        <div className="mt-4 flex items-center gap-2">
          <Button onClick={handleSync} disabled={syncing}>
            <RefreshCw className={`w-3.5 h-3.5 ${syncing ? 'animate-spin' : ''}`} />
            {t('hosts.syncButton')}
          </Button>
          <span className="text-[11px] text-text-tertiary">{t('hosts.syncHint')}</span>
        </div>
      </div>

      {/* Create Dialog */}
      {formDialog(showCreate, () => setShowCreate(false), t('hosts.createTitle'), handleCreate, t('hosts.create'))}

      {/* Edit Dialog */}
      {formDialog(!!editTarget, () => setEditTarget(null), t('hosts.editTitle'), handleUpdate, t('hosts.save'))}

      {/* Delete Confirm */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('common.delete')}
        message={t('hosts.deleteConfirm', { hostname: deleteTarget?.hostname })}
        confirmText={t('common.delete')}
        danger
      />
    </>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/pages/HostsPage.tsx
git commit -m "feat(hosts): add HostsPage component with table, CRUD dialogs, and sync button"
```

---

### Task 11: Router & Sidebar Integration

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Add route to App.tsx**

In `src/App.tsx`, add import:

```typescript
import { HostsPage } from "./pages/HostsPage";
```

Add route after the `/access` route:

```tsx
<Route path="/hosts" element={<HostsPage />} />
```

- [ ] **Step 2: Add sidebar nav item**

In `src/components/layout/Sidebar.tsx`, add `Globe` to the lucide-react imports:

```typescript
import {
  BarChart3,
  Activity,
  Lock,
  Shield,
  Globe,
  ClipboardList,
  Settings,
} from 'lucide-react';
```

Add `useHostsStore` import:

```typescript
import { useHostsStore } from '../../stores/hosts-store';
```

Add hosts count selector after `accessCount`:

```typescript
const hostsCount = useHostsStore((s) => s.entries.length);
```

Add the Hosts nav item to the Security section, after the Access Control item:

```typescript
{ icon: Globe, labelKey: 'nav.hosts', path: '/hosts', count: hostsCount || undefined },
```

- [ ] **Step 3: Verify the frontend builds**

Run: `cd /Volumes/work/mpan/projects/proxy-manager && npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx src/components/layout/Sidebar.tsx
git commit -m "feat(hosts): add route and sidebar navigation entry"
```

---

### Task 12: Proxy Form Smart Prompt (Create)

**Files:**
- Modify: `src/components/proxy/ProxyForm.tsx`

- [ ] **Step 1: Add imports and state for hosts prompt**

In `src/components/proxy/ProxyForm.tsx`, add import:

```typescript
import { Dialog } from '../ui/Dialog';
import { checkHostnameExists, createHost } from '../../lib/api';
```

Add state variables inside the `ProxyForm` component (after the existing `errors` state):

```typescript
const [showHostsPrompt, setShowHostsPrompt] = useState(false);
const [hostsPromptDomain, setHostsPromptDomain] = useState('');
const [hostsPromptIp, setHostsPromptIp] = useState('127.0.0.1');
```

- [ ] **Step 2: Add post-save hosts check logic**

In the `handleSave` function, replace the create branch's success handling. Currently it reads:

```typescript
await createProxy(input);
addToast('success', t('proxyForm.createSuccess'));
```

Replace with:

```typescript
await createProxy(input);
addToast('success', t('proxyForm.createSuccess'));

// Check if domain needs a hosts entry
if (domain.trim() && !isStream) {
  try {
    const existing = await checkHostnameExists(domain.trim());
    if (!existing) {
      setHostsPromptDomain(domain.trim());
      setHostsPromptIp('127.0.0.1');
      setShowHostsPrompt(true);
      return; // Don't navigate yet
    }
  } catch { /* ignore check failure */ }
}
```

- [ ] **Step 3: Add the hosts prompt Dialog JSX**

Add before the closing `</div>` of the component return:

```tsx
{/* Hosts entry prompt after proxy creation */}
<Dialog
  open={showHostsPrompt}
  onClose={() => { setShowHostsPrompt(false); navigate('/'); }}
  title={t('hosts.promptTitle')}
  footer={
    <>
      <Button onClick={() => { setShowHostsPrompt(false); navigate('/'); }}>
        {t('hosts.promptSkip')}
      </Button>
      <Button
        variant="primary"
        onClick={async () => {
          try {
            await createHost({ ip: hostsPromptIp, hostname: hostsPromptDomain });
            addToast('success', t('hosts.createSuccess'));
          } catch (e) {
            addToast('error', String(e));
          }
          setShowHostsPrompt(false);
          navigate('/');
        }}
      >
        {t('hosts.promptAdd')}
      </Button>
    </>
  }
>
  <p className="text-[13px] text-text-secondary mb-3">
    {t('hosts.promptMessage', { domain: hostsPromptDomain })}
  </p>
  <div>
    <label className="block text-[12px] font-medium text-text-secondary mb-1">
      {t('hosts.ip')}
    </label>
    <Input
      value={hostsPromptIp}
      onChange={(e) => setHostsPromptIp(e.target.value)}
      placeholder="127.0.0.1"
    />
  </div>
</Dialog>
```

- [ ] **Step 4: Verify the frontend builds**

Run: `cd /Volumes/work/mpan/projects/proxy-manager && npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/components/proxy/ProxyForm.tsx
git commit -m "feat(hosts): add smart hosts prompt on proxy creation"
```

---

### Task 13: Proxy Delete Hosts Cleanup Prompt

**Files:**
- Modify: `src/pages/DashboardPage.tsx`

- [ ] **Step 1: Read DashboardPage.tsx to find the delete handler**

Read the current file to understand the delete flow before making changes.

- [ ] **Step 2: Add hosts cleanup logic to delete handler**

This task depends on the exact structure of DashboardPage.tsx. The integration should:

1. Import `checkHostnameExists` and `deleteHost` from `../../lib/api`
2. After `deleteProxy(id)` succeeds, check if the deleted proxy's domain has other proxy rules using it
3. If no other rules use the domain and a hosts entry exists, show a confirmation dialog
4. On confirm, call `deleteHost(hostEntry.id)`

The exact code depends on the current DashboardPage structure, which the implementing agent should read first. The pattern follows the ProxyForm integration in Task 12: add state for a cleanup dialog, check after delete, show dialog conditionally.

Key logic:

```typescript
// After successful delete of a proxy with domain:
if (deletedProxy.domain) {
  const allProxies = await listProxies();
  const domainStillUsed = allProxies.rules.some(
    (r) => r.domain === deletedProxy.domain && r.id !== deletedProxy.id
  );
  if (!domainStillUsed) {
    const hostEntry = await checkHostnameExists(deletedProxy.domain);
    if (hostEntry) {
      // Show cleanup dialog
    }
  }
}
```

- [ ] **Step 3: Verify the frontend builds**

Run: `cd /Volumes/work/mpan/projects/proxy-manager && npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/pages/DashboardPage.tsx
git commit -m "feat(hosts): add hosts cleanup prompt on proxy deletion"
```

---

### Task 14: End-to-End Verification

**Files:** None (testing only)

- [ ] **Step 1: Build the full Rust backend**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo build 2>&1 | tail -10`
Expected: BUILD SUCCEEDED

- [ ] **Step 2: Run all Rust tests**

Run: `cd /Volumes/work/mpan/projects/proxy-manager/src-tauri && cargo test 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 3: TypeScript type check**

Run: `cd /Volumes/work/mpan/projects/proxy-manager && npx tsc --noEmit 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 4: Frontend dev build check**

Run: `cd /Volumes/work/mpan/projects/proxy-manager && npx vite build 2>&1 | tail -10`
Expected: build succeeds

- [ ] **Step 5: Commit any fixes needed**

If any verification step fails, fix and commit.
