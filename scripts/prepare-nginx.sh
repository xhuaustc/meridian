#!/usr/bin/env bash
# Download and compile nginx from source for Tauri sidecar bundling.
#
# Produces a self-contained nginx binary with OpenSSL, zlib and PCRE2 statically
# linked. No system library dependencies beyond libc and system frameworks.
#
# PCRE2 is required by the rewrite module (used for the HTTP-to-HTTPS redirect).
# It is built from source on purpose: letting nginx probe the system would link
# whatever PCRE happens to be installed on the build machine.
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
#   PCRE2_VERSION   (default: 10.44)
#   JOBS            (default: auto-detect CPU count)
#   FORCE_REBUILD   (set to 1 to rebuild even if binary exists)
#
# Changing any configure flag (including module flags) requires FORCE_REBUILD=1,
# otherwise the script exits early with the previously built binary in place.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"
BUILD_DIR="$PROJECT_DIR/.nginx-build"

# Versions
NGINX_VERSION="${NGINX_VERSION:-1.26.2}"
OPENSSL_VERSION="${OPENSSL_VERSION:-3.3.2}"
ZLIB_VERSION="${ZLIB_VERSION:-1.3.1}"
PCRE2_VERSION="${PCRE2_VERSION:-10.44}"

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
PCRE2_URL="https://github.com/PCRE2Project/pcre2/releases/download/pcre2-${PCRE2_VERSION}/pcre2-${PCRE2_VERSION}.tar.gz"

# Pinned checksums. Bumping a version means bumping its checksum here as well;
# the build refuses to proceed on a mismatch rather than trusting the download.
NGINX_SHA256="${NGINX_SHA256:-627fe086209bba80a2853a0add9d958d7ebbdffa1a8467a5784c9a6b4f03d738}"
OPENSSL_SHA256="${OPENSSL_SHA256:-2e8a40b01979afe8be0bbfb3de5dc1c6709fedb46d6c89c10da114ab5fc3d281}"
ZLIB_SHA256="${ZLIB_SHA256:-9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23}"
PCRE2_SHA256="${PCRE2_SHA256:-86b9cb0aa3bcb7994faa88018292bc704cdbb708e785f7c74352ff6ea7d3175b}"

echo "=== Meridian nginx builder ==="
echo "nginx:   ${NGINX_VERSION}"
echo "openssl: ${OPENSSL_VERSION}"
echo "zlib:    ${ZLIB_VERSION}"
echo "pcre2:   ${PCRE2_VERSION}"
echo "target:  ${TARGET}"
echo "jobs:    ${MAKE_JOBS}"
echo ""

# Create build directory
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Helper: download and extract, verifying the archive against a pinned checksum
fetch_and_extract() {
    local name="$1" url="$2" dir="$3" expected_sha256="$4"
    if [ -d "$dir" ]; then
        echo "[$name] Already extracted: $dir"
        return
    fi
    local archive="${dir}.tar.gz"
    if [ ! -f "$archive" ]; then
        echo "[$name] Downloading..."
        curl -fSL --retry 3 -o "$archive" "$url"
    fi
    verify_sha256 "$name" "$archive" "$expected_sha256"
    echo "[$name] Extracting..."
    tar xzf "$archive"
}

# Helper: fail loudly when an archive does not match its pinned checksum
verify_sha256() {
    local name="$1" file="$2" expected="$3"
    local actual
    if command -v sha256sum &>/dev/null; then
        actual="$(sha256sum "$file" | cut -d' ' -f1)"
    else
        actual="$(shasum -a 256 "$file" | cut -d' ' -f1)"
    fi
    if [ "$actual" != "$expected" ]; then
        echo "[$name] Checksum mismatch for $file" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        echo "  Refusing to build. Delete the archive and retry, or update the pinned checksum." >&2
        exit 1
    fi
}

# Download all sources in parallel
fetch_and_extract "nginx"   "$NGINX_URL"   "nginx-${NGINX_VERSION}"     "$NGINX_SHA256"
fetch_and_extract "openssl" "$OPENSSL_URL" "openssl-${OPENSSL_VERSION}" "$OPENSSL_SHA256"
fetch_and_extract "zlib"    "$ZLIB_URL"    "zlib-${ZLIB_VERSION}"      "$ZLIB_SHA256"
fetch_and_extract "pcre2"   "$PCRE2_URL"   "pcre2-${PCRE2_VERSION}"    "$PCRE2_SHA256"

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
    --prefix=.
    --sbin-path=nginx
    --error-log-path=logs/error.log
    --pid-path=nginx.pid
    --http-client-body-temp-path=temp/client_body
    --http-proxy-temp-path=temp/proxy
    --http-log-path=logs/access.log

    # Statically link dependencies (source tree paths)
    --with-openssl="../openssl-${OPENSSL_VERSION}"
    --with-zlib="../zlib-${ZLIB_VERSION}"
    --with-pcre="../pcre2-${PCRE2_VERSION}"

    # Required modules for Meridian proxy manager
    --with-http_ssl_module
    --with-http_v2_module
    --with-stream
    --with-stream_ssl_module
    --with-stream_ssl_preread_module

    # Disable unused modules to minimize binary size
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
    --without-http_upstream_zone_module
    --without-stream_geo_module
    --without-stream_split_clients_module
    --without-stream_return_module
    --without-stream_set_module
    --without-stream_upstream_hash_module
    --without-stream_upstream_least_conn_module
    --without-stream_upstream_zone_module
)

# Apple Clang 16+ warns about unterminated string initializers (nginx 1.26.2 has
# one in its HTTP/2 filter module) and nginx builds with -Werror, so the module
# fails to compile unless the warning is silenced. Scope it to macOS: GCC on
# Linux does not know this option.
if [ "$(uname -s)" = "Darwin" ]; then
    CONFIGURE_ARGS+=(--with-cc-opt="-Wno-unterminated-string-initialization")
fi

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
