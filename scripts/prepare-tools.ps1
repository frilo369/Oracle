# Reuse the owner's already installed tools; no application source is modified.
param([string]$Reference='C:\Users\fryil\Documents\Rust-Friloverse-Skins')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$destination=Join-Path $root 'dependencies'
New-Item -ItemType Directory -Force $destination|Out-Null
$runtime=Join-Path $Reference 'dependencies/MicrosoftEdgeWebview2Setup.exe'
$signature=Get-AuthenticodeSignature -LiteralPath $runtime
if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)'){throw 'Expected an official Microsoft-signed WebView2 bootstrapper'}
Copy-Item -LiteralPath $runtime -Destination (Join-Path $destination 'MicrosoftEdgeWebview2Setup.exe')
$browser=Join-Path $Reference 'dependencies/lobby-browser/node_modules/playwright-core'
if(!(Test-Path -LiteralPath (Join-Path $browser 'index.mjs'))){throw 'Reference Playwright Core package is unavailable'}
$package=Join-Path $destination 'playwright-core'
New-Item -ItemType Directory -Force $package|Out-Null
Get-ChildItem -LiteralPath $browser -Force|Copy-Item -Destination $package -Recurse -Force
Write-Output 'Oracle build and acceptance tools are ready.'
