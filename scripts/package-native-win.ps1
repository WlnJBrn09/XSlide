# Package XSlide as a portable native Windows folder + zip.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$pkg = Get-Content (Join-Path $root "package.json") -Raw | ConvertFrom-Json
$version = $pkg.version
$product = "XSlide"
$outName = "${product}_v${version}_win"
$dist = Join-Path $root "dist"
$stage = Join-Path $dist $outName

Write-Host "Building backend + native host..."
cargo build --release
cargo build --release --manifest-path (Join-Path $root "native-host\Cargo.toml")
node (Join-Path $root "scripts\make-icon.js")

if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "backend") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "build") | Out-Null

Copy-Item (Join-Path $root "native-host\target\release\xslide-native.exe") (Join-Path $stage "XSlide.exe")
Copy-Item (Join-Path $root "target\release\xslide.exe") (Join-Path $stage "backend\xslide.exe")
Copy-Item -Recurse (Join-Path $root "static") (Join-Path $stage "static")
if (Test-Path (Join-Path $root "build\icon.ico")) {
  Copy-Item (Join-Path $root "build\icon.ico") (Join-Path $stage "build\icon.ico")
}
if (Test-Path (Join-Path $root "build\icon.png")) {
  Copy-Item (Join-Path $root "build\icon.png") (Join-Path $stage "build\icon.png")
}

$zip = Join-Path $dist "${outName}.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -Force
$size = (Get-Item $zip).Length
Write-Host "Packed $zip ($size bytes)"
Write-Host "STAGE=$stage"
Write-Host "ZIP=$zip"
