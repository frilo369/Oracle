param([Parameter(Mandatory=$true)][string]$Version)
$ErrorActionPreference='Stop'
if($Version -notmatch '^([1-9][0-9]*\.[0-9])(?:\.0)?$'){throw 'Use major.minor with one minor digit, such as 1.3'}
$package=$Matches[1]+'.0'
$root=Split-Path $PSScriptRoot -Parent
$cargo=Join-Path $root 'src-tauri/Cargo.toml'
$config=Join-Path $root 'src-tauri/tauri.conf.json'
[IO.File]::WriteAllText($cargo,([IO.File]::ReadAllText($cargo) -replace '(?m)^version = "[^"]+"',('version = "'+$package+'"')))
[IO.File]::WriteAllText($config,([IO.File]::ReadAllText($config) -replace '"version": "[^"]+"',('"version": "'+$package+'"')))
& "$PSScriptRoot/cargo.ps1" check --offline
Write-Output "Oracle version set to $package"
