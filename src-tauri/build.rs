fn main() {
    // Warn if sidecar nginx binary is missing for the current target
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let sidecar = format!("binaries/nginx-{}{}", target, ext);
    if !std::path::Path::new(&sidecar).exists() {
        println!(
            "cargo:warning=nginx sidecar binary not found at '{}'. \
             Run ./scripts/prepare-nginx.sh (Unix) or .\\scripts\\prepare-nginx.ps1 (Windows) before building for release.",
            sidecar
        );
    }

    tauri_build::build()
}
