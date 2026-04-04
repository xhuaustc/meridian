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
