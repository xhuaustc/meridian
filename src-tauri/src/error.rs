use thiserror::Error;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Certificate error: {0}")]
    Certificate(String),

    #[error("Nginx error: {0}")]
    Nginx(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("DNS provider error: {0}")]
    Dns(String),

    #[error("ACME error: {0}")]
    Acme(String),
}

impl AppError {
    pub fn error_code(&self) -> String {
        match self {
            AppError::Database(_) => "DB_ERROR".to_string(),
            AppError::Io(_) => "IO_ERROR".to_string(),
            AppError::Json(_) => "JSON_ERROR".to_string(),
            AppError::Certificate(_) => "CERT_ERROR".to_string(),
            AppError::Nginx(_) => "NGINX_ERROR".to_string(),
            AppError::Config(_) => "CONFIG_ERROR".to_string(),
            AppError::Conflict(_) => "CONFLICT".to_string(),
            AppError::NotFound(_) => "NOT_FOUND".to_string(),
            AppError::Validation(msg) => {
                // Generate specific codes from validation messages
                if msg.contains("Name") { "VALIDATION_NAME".to_string() }
                else if msg.contains("listen_port") { "VALIDATION_PORT".to_string() }
                else if msg.contains("upstream_port") { "VALIDATION_UPSTREAM_PORT".to_string() }
                else if msg.contains("domain") { "VALIDATION_DOMAIN".to_string() }
                else if msg.contains("proxy_type") { "VALIDATION_PROXY_TYPE".to_string() }
                else if msg.contains("tls_mode") { "VALIDATION_TLS_MODE".to_string() }
                else if msg.contains("certificate_id") { "VALIDATION_CERTIFICATE".to_string() }
                else if msg.contains("websocket") { "VALIDATION_WEBSOCKET".to_string() }
                else if msg.contains("path_prefix") { "VALIDATION_PATH_PREFIX".to_string() }
                else if msg.contains("IP") || msg.contains("CIDR") { "VALIDATION_IP".to_string() }
                else if msg.contains("Hostname") || msg.contains("hostname") { "VALIDATION_HOSTNAME".to_string() }
                else { "VALIDATION_ERROR".to_string() }
            }
            AppError::Dns(_) => "DNS_ERROR".to_string(),
            AppError::Acme(_) => "ACME_ERROR".to_string(),
        }
    }
}

impl From<AppError> for tauri::ipc::InvokeError {
    fn from(err: AppError) -> Self {
        let response = ErrorResponse {
            code: err.error_code(),
            message: err.to_string(),
        };
        // Serialize as JSON string so frontend can parse it
        let json = serde_json::to_string(&response)
            .unwrap_or_else(|_| format!(r#"{{"code":"UNKNOWN","message":"{}"}}"#, err));
        tauri::ipc::InvokeError::from(json)
    }
}
