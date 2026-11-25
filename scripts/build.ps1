$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
& "$PSScriptRoot/cargo.ps1" build --locked --release --offline
if ($LASTEXITCODE) { throw 'Release build failed' }
$version=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version -replace '\.0$',''
$destination=Join-Path $root "dist/$version"
New-Item -ItemType Directory -Force $destination|Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target/release/oracle.exe') -Destination (Join-Path $destination 'Oracle.exe')
Write-Output "Oracle $version : $(Join-Path $destination 'Oracle.exe')"
