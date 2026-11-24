# Season Desktop - build MSI
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$wixDirs = @()
if ($env:WIX) { $wixDirs += (Join-Path $env:WIX "bin") }
$candleOnPath = Get-Command candle.exe -ErrorAction SilentlyContinue
if ($candleOnPath) { $wixDirs += (Split-Path $candleOnPath.Source) }
$wixDirs += "$env:LOCALAPPDATA\tauri\WixTools314"
$wixDirs += "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin"
$wix = $wixDirs | Where-Object {
    $_ -and (Test-Path -LiteralPath (Join-Path $_ "candle.exe") -PathType Leaf) -and
    (Test-Path -LiteralPath (Join-Path $_ "light.exe") -PathType Leaf)
} | Select-Object -First 1
if (-not $wix) {
    throw "Cần WiX Toolset 3.14 (candle.exe và light.exe). Xem README.md."
}

Write-Host "1/2 Build Season Desktop $version..." -ForegroundColor Cyan
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build lỗi" }

Write-Host "2/2 Tạo bộ cài MSI..." -ForegroundColor Cyan
$dist = Join-Path $root "dist"
$obj = Join-Path $root "target\wix"
New-Item -ItemType Directory -Force $dist, $obj | Out-Null
$msi = Join-Path $dist "SeasonDesktop-$version-x64.msi"
$wixobj = Join-Path $obj "season-desktop.wixobj"
& (Join-Path $wix "candle.exe") -nologo -arch x64 "-dAppVersion=$version" "-dSourceDir=$root" -ext WixUIExtension -out $wixobj "installer\season-desktop.wxs"
if ($LASTEXITCODE -ne 0) { throw "candle lỗi" }
& (Join-Path $wix "light.exe") -nologo -ext WixUIExtension -cultures:en-us -sice:ICE38 -sice:ICE64 -sice:ICE91 -spdb -out $msi $wixobj
if ($LASTEXITCODE -ne 0) { throw "light lỗi" }
Write-Host "   -> $msi" -ForegroundColor Green
