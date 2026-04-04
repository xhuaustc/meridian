#!/usr/bin/env bash
# Download and compile nginx from source for Tauri sidecar bundling.
#
# Produces a self-contained nginx binary with OpenSSL and zlib statically linked.
# No system library dependencies beyond libc and system frameworks.
#
# Supported platforms:
#   macOS  (aarch64-apple-darwin, x86_64-apple-darwin)
#   Linux  (x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu)
#
# Prerequisites:
#   macOS: Xcode Command Line Tools (xcode-select --install)
#   Linux: build-essential (gcc, make) — install via:
#     apt:  sudo apt install build-essential
#     yum:  sudo yum groupinstall "Development Tools"
#
# Usage:
#   ./scripts/prepare-nginx.sh                          # native build
#   ./scripts/prepare-nginx.sh aarch64-apple-darwin      # explicit target
#
# Environment variables:
#   NGINX_VERSION   (default: 1.26.2)
#   OPENSSL_VERSION (default: 3.3.2)
#   ZLIB_VERSION    (default: 1.3.1)
#   JOBS            (default: auto-detect CPU count)
#   FORCE_REBUILD   (set to 1 to rebuild even if binary exists)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"
BUILD_DIR="$PROJECT_DIR/.nginx-build"

# Versions
NGINX_VERSION="${NGINX_VERSION:-1.26.2}"
OPENSSL_VERSION="${OPENSSL_VERSION:-3.3.2}"
ZLIB_VERSION="${ZLIB_VERSION:-1.3.1}"

# Target triple
if [ -n "${1:-}" ]; then
    TARGET="$1"
else
    TARGET="$(rustc -vV | grep '^host:' | cut -d' ' -f2)"
fi

DEST="$BINARIES_DIR/nginx-$TARGET"

# Skip if already built (unless FORCE_REBUILD)
if [ -f "$DEST" ] && [ "${FORCE_REBUILD:-0}" != "1" ]; then
    echo "nginx sidecar already exists at $DEST"
    echo "Set FORCE_REBUILD=1 to rebuild."
    exit 0
fi

# Parallel jobs
if [ -n "${JOBS:-}" ]; then
    MAKE_JOBS="$JOBS"
elif command -v nproc &>/dev/null; then
    MAKE_JOBS="$(nproc)"
elif command -v sysctl &>/dev/null; then
    MAKE_JOBS="$(sysctl -n hw.ncpu)"
else
    MAKE_JOBS=4
fi

# Download URLs
NGINX_URL="https://nginx.org/download/nginx-${NGINX_VERSION}.tar.gz"
OPENSSL_URL="https://github.com/openssl/openssl/releases/download/openssl-${OPENSSL_VERSION}/openssl-${OPENSSL_VERSION}.tar.gz"
ZLIB_URL="https://github.com/madler/zlib/releases/download/v${ZLIB_VERSION}/zlib-${ZLIB_VERSION}.tar.gz"

echo "=== Meridian nginx builder ==="
echo "nginx:   ${NGINX_VERSION}"
echo "openssl: ${OPENSSL_VERSION}"
echo "zlib:    ${ZLIB_VERSION}"
echo "target:  ${TARGET}"
echo "jobs:    ${MAKE_JOBS}"
echo ""

# Create build directory
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Helper: download and extract
fetch_and_extract() {
    local name="$1" url="$2" dir="$3"
    if [ -d "$dir" ]; then
        echo "[$name] Already extracted: $dir"
        return
    fi
    local archive="${dir}.tar.gz"
    if [ ! -f "$archive" ]; then
        echo "[$name] Downloading..."
        curl -fSL --retry 3 -o "$archive" "$url"
    fi
    echo "[$name] Extracting..."
    tar xzf "$archive"
}

# Download all sources in parallel
fetch_and_extract "nginx"   "$NGINX_URL"   "nginx-${NGINX_VERSION}"
fetch_and_extract "openssl" "$OPENSSL_URL" "openssl-${OPENSSL_VERSION}"
fetch_and_extract "zlib"    "$ZLIB_URL"    "zlib-${ZLIB_VERSION}"

# Configure nginx
cd "nginx-${NGINX_VERSION}"

# Clean previous build artifacts if force rebuilding
if [ "${FORCE_REBUILD:-0}" = "1" ] && [ -f Makefile ]; then
    echo "=== Cleaning previous build ==="
    make clean 2>/dev/null || true
fi

echo ""
echo "=== Configuring nginx ==="

CONFIGURE_ARGS=(
    --prefix=/etc/nginx
    --sbin-path=nginx

    # Statically link dependencies (source tree paths)
    --with-openssl="../openssl-${OPENSSL_VERSION}"
    --with-zlib="../zlib-${ZLIB_VERSION}"

    # Required modules for Meridian proxy manager
    --with-http_ssl_module
    --with-stream
    --with-stream_ssl_module
    --with-stream_ssl_preread_module

    # Disable unused modules to minimize binary size
    --without-http_rewrite_module
    --without-http_gzip_module
    --without-http_fastcgi_module
    --without-http_uwsgi_module
    --without-http_scgi_module
    --without-http_memcached_module
    --without-http_empty_gif_module
    --without-http_browser_module
    --without-http_autoindex_module
    --without-http_geo_module
    --without-http_split_clients_module
    --without-http_ssi_module
    --without-http_userid_module
    --without-http_mirror_module
    --without-http_auth_basic_module
    --without-mail_pop3_module
    --without-mail_imap_module
    --without-mail_smtp_module
    --without-http_charset_module
    --without-http_upstream_hash_module
    --without-http_upstream_ip_hash_module
    --without-http_upstream_least_conn_module
    --without-http_upstream_keepalive_module
    --without-http_upstream_zone_module
    --without-stream_geo_module
    --without-stream_split_clients_module
    --without-stream_return_module
    --without-stream_set_module
    --without-stream_upstream_hash_module
    --without-stream_upstream_least_conn_module
    --without-stream_upstream_zone_module
)

./configure "${CONFIGURE_ARGS[@]}"

echo ""
echo "=== Building nginx ==="
make -j"${MAKE_JOBS}"

# Copy the binary
mkdir -p "$BINARIES_DIR"
cp objs/nginx "$DEST"
chmod +x "$DEST"

# Show result
echo ""
echo "=== Done ==="
ls -lh "$DEST"
file "$DEST"
echo ""
echo "nginx ${NGINX_VERSION} built and placed at: $DEST"
