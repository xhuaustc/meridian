use std::fs;
use std::path::Path;

use crate::error::AppError;

/// Write built-in error page HTML files to the nginx/html directory.
pub fn write_error_pages(data_dir: &Path) -> Result<(), AppError> {
    let html_dir = data_dir.join("nginx/html");
    fs::create_dir_all(&html_dir)?;
    fs::write(html_dir.join("502.html"), ERROR_502_HTML)?;
    Ok(())
}

const ERROR_502_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>502 Bad Gateway</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    background: #fafaf9;
    color: #1c1917;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: 24px;
  }
  .container {
    text-align: center;
    max-width: 480px;
  }
  .icon {
    width: 64px;
    height: 64px;
    margin: 0 auto 24px;
    border-radius: 16px;
    background: #fef2f2;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .icon svg {
    width: 32px;
    height: 32px;
    color: #dc2626;
  }
  h1 {
    font-size: 20px;
    font-weight: 600;
    margin-bottom: 8px;
  }
  .code {
    font-size: 14px;
    color: #78716c;
    margin-bottom: 16px;
  }
  p {
    font-size: 14px;
    color: #78716c;
    line-height: 1.6;
    margin-bottom: 24px;
  }
  .actions {
    display: flex;
    gap: 12px;
    justify-content: center;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    border-radius: 6px;
    border: 1px solid #e7e5e4;
    background: #fff;
    color: #1c1917;
    cursor: pointer;
    text-decoration: none;
    transition: background 150ms ease;
  }
  .btn:hover { background: #f5f5f4; }
  .btn-primary {
    background: #2563eb;
    color: #fff;
    border-color: #2563eb;
  }
  .btn-primary:hover { background: #1d4ed8; }
  .footer {
    margin-top: 48px;
    font-size: 12px;
    color: #a8a29e;
  }
</style>
</head>
<body>
  <div class="container">
    <div class="icon">
      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
      </svg>
    </div>
    <h1>Bad Gateway</h1>
    <div class="code">HTTP 502</div>
    <p>
      The upstream server is not responding. This usually means the
      target service is down, still starting up, or unreachable.
    </p>
    <div class="actions">
      <a class="btn btn-primary" href="javascript:location.reload()">Retry</a>
    </div>
    <div class="footer">Meridian Proxy Manager</div>
  </div>
</body>
</html>
"##;
