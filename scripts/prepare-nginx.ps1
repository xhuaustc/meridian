# Download official nginx Windows binary for Tauri sidecar bundling.
#
# nginx.org provides pre-built Windows binaries (x86_64 only).
# Downloads the zip, extracts nginx.exe, and places it in the sidecar location.
#
# Usage:
#   .\scripts\prepare-nginx.ps1
#   .\scripts\prepare-nginx.ps1 -NginxVersion "1.26.2"
#   .\scripts\prepare-nginx.ps1 -ForceRebuild
#
param(
    [string]$NginxVersion = "1.26.2",
    [string]$Target = "",
    [switch]$ForceRebuild
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$BinariesDir = Join-Path $ProjectDir "src-tauri\binaries"
$BuildDir = Join-Path $ProjectDir ".nginx-build"

# Auto-detect target triple
if (-not $Target) {
    $Target = & rustc -vV | Select-String "^host:" | ForEach-Object { ($_ -split "\s+")[1] }
}

$Dest = Join-Path $BinariesDir "nginx-$Target.exe"

# Skip if already exists
if ((Test-Path $Dest) -and -not $ForceRebuild) {
    Write-Host "nginx sidecar already exists at $Dest"
    Write-Host "Use -ForceRebuild to rebuild."
    exit 0
}

$NginxUrl = "https://nginx.org/download/nginx-$NginxVersion.zip"

Write-Host "=== Meridian nginx builder (Windows) ==="
Write-Host "nginx:   $NginxVersion"
Write-Host "target:  $Target"
Write-Host ""

# Create build directory
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
$ZipFile = Join-Path $BuildDir "nginx-$NginxVersion.zip"
$ExtractDir = Join-Path $BuildDir "nginx-$NginxVersion"

# Download
if (-not (Test-Path $ZipFile)) {
    Write-Host "[nginx] Downloading from $NginxUrl ..."
    Invoke-WebRequest -Uri $NginxUrl -OutFile $ZipFile -UseBasicParsing
}

# Extract
if (-not (Test-Path $ExtractDir)) {
    Write-Host "[nginx] Extracting..."
    Expand-Archive -Path $ZipFile -DestinationPath $BuildDir -Force
}

# Copy nginx.exe to sidecar location
$NginxExe = Join-Path $ExtractDir "nginx.exe"
if (-not (Test-Path $NginxExe)) {
    Write-Error "nginx.exe not found at $NginxExe after extraction."
    exit 1
}

New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
Copy-Item $NginxExe $Dest -Force

Write-Host ""
Write-Host "=== Done ==="
Write-Host "nginx $NginxVersion placed at: $Dest"
Get-Item $Dest | Format-List Name, Length, LastWriteTime
