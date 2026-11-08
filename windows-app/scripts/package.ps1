<#
.SYNOPSIS
    Builds a release Walkman Studio binary and stages a distributable folder under
    windows-app/dist/<target>/ (spec task 19.1).

.DESCRIPTION
    Real asset bundling, scoped to what this repo can actually supply without a licensed
    or network-fetched binary:
      - Device images and SensMe channel covers already exist at the repo root (shared with
        the macOS app) and are copied in as-is.
      - Bundled ffmpeg.exe/ffprobe.exe are copied in from windows-app/assets/bin/ if present
        there (per assets/README.md); `ffmpeg_util::find_ffmpeg`/`find_ffprobe` (task 8.3)
        already prefer this location at runtime, then fall back to PATH.
      - Segoe UI (not redistributable) and the Lucide icon font (would need a network
        fetch this environment doesn't have) are NOT bundled by this script. Their absence
        is not a build failure: `ui::theme::install_fonts`/`bundled_icon_font` already fall
        back to egui's built-in fonts and each `Icon`'s `fallback_text` respectively (see
        assets/README.md) — this script just reports that they're missing rather than
        fabricating them.

.PARAMETER Target
    The Rust target triple to build for: "x64" (x86_64-pc-windows-msvc, the default) or
    "arm64" (aarch64-pc-windows-msvc). Building arm64 on an x64 dev host needs the ARM64
    MSVC C++ Build Tools component (rusqlite's bundled SQLite and `ring` both need a C
    cross-compiler for the target); without it this script's build step fails with a clear
    "clang/cl.exe not found" error from `cargo build` rather than silently producing a
    wrong-architecture binary.

.EXAMPLE
    ./scripts/package.ps1 -Target x64
#>
param(
    [ValidateSet("x64", "arm64")]
    [string]$Target = "x64"
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$appRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$triple = if ($Target -eq "arm64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }

Write-Host "Building walkman-studio (release, $triple)..."
Push-Location $appRoot
try {
    cargo build -p walkman-studio --release --target $triple
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed for $triple (exit $LASTEXITCODE). For arm64 this almost always means the ARM64 MSVC C++ Build Tools component isn't installed on this host: rusqlite's bundled SQLite and the ring crate both need a C cross-compiler for the target, and cargo build's own output above will show a 'clang'/'cl.exe' not found error from cc-rs if that's the cause."
    }
} finally {
    Pop-Location
}

$distDir = Join-Path $appRoot "dist\$Target"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $distDir "assets\devices") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $distDir "assets\senseme") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $distDir "assets\bin") | Out-Null

$exeSource = Join-Path $appRoot "target\$triple\release\walkman-studio.exe"
Copy-Item $exeSource -Destination $distDir -Force
Write-Host "Staged $exeSource -> $distDir"

$deviceSource = Join-Path $repoRoot "assets\devices"
if (Test-Path $deviceSource) {
    Copy-Item (Join-Path $deviceSource "*.png") -Destination (Join-Path $distDir "assets\devices") -Force
    $count = (Get-ChildItem (Join-Path $distDir "assets\devices") -Filter *.png).Count
    Write-Host "Staged $count device image(s)."
} else {
    Write-Warning "No device images found at $deviceSource; Device Overview's model art stays a placeholder tile."
}

$senseMeSource = Join-Path $repoRoot "assets\SensMe Channel cover"
if (Test-Path $senseMeSource) {
    Get-ChildItem $senseMeSource -Filter *.jpg | ForEach-Object {
        # Slug convention for a future loader: lowercase, spaces -> underscores, matching
        # `SenseMeChannel::display_name()`'s own text (e.g. "Shuffle all songs.jpg" ->
        # "shuffle_all_songs.jpg").
        $slug = ($_.BaseName.ToLowerInvariant() -replace '[^a-z0-9]+', '_').Trim('_')
        Copy-Item $_.FullName -Destination (Join-Path $distDir "assets\senseme\$slug.jpg") -Force
    }
    $count = (Get-ChildItem (Join-Path $distDir "assets\senseme") -Filter *.jpg).Count
    Write-Host "Staged $count SensMe channel cover(s)."
} else {
    Write-Warning "No SensMe channel covers found at $senseMeSource; the carousel stays on its glass-tile placeholder."
}

$bundledBin = Join-Path $appRoot "assets\bin"
$ffmpegStaged = $false
foreach ($name in @("ffmpeg.exe", "ffprobe.exe")) {
    $source = Join-Path $bundledBin $name
    if (Test-Path $source) {
        Copy-Item $source -Destination (Join-Path $distDir "assets\bin") -Force
        $ffmpegStaged = $true
    }
}
if ($ffmpegStaged) {
    Write-Host "Staged bundled ffmpeg/ffprobe from $bundledBin."
} else {
    Write-Warning "No ffmpeg.exe/ffprobe.exe at $bundledBin (see assets/README.md); the shipped build will fall back to PATH at runtime (ffmpeg_util::find_ffmpeg/find_ffprobe, task 8.3)."
}

Write-Warning "Segoe UI and the Lucide icon font are NOT bundled by this script (not redistributable / needs a network fetch); install_fonts/bundled_icon_font already fall back to egui's built-in fonts and each Icon's fallback_text, so this is a cosmetic default, not a broken build."

Write-Host "`nPackaged $Target build at $distDir"
