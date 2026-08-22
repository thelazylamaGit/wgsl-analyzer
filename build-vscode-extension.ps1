$ErrorActionPreference = "Stop"

$repository = $PSScriptRoot
$rustTarget = "x86_64-pc-windows-msvc"
$server = Join-Path $repository "target\$rustTarget\release\wgsl-analyzer.exe"
$bundledServer = Join-Path $repository "editors\code\server\wgsl-analyzer.exe"
$vsix = Join-Path $repository "editors\code\wgsl-analyzer-win32-x64.vsix"

Push-Location $repository
try {
	cargo build --release --target $rustTarget --bin wgsl-analyzer
	if ($LASTEXITCODE -ne 0) { throw "Failed to build wgsl-analyzer" }

	New-Item -ItemType Directory -Force (Split-Path $bundledServer) | Out-Null
	Copy-Item -Force $server $bundledServer

	pnpm.cmd --dir editors/code run package --target win32-x64 -o $vsix
	if ($LASTEXITCODE -ne 0) { throw "Failed to package the VS Code extension" }

	Write-Host "Created $vsix"
}
finally {
	Pop-Location
}
