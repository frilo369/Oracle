param([string]$Setup='')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$Setup){$version=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version -replace '\.0$','';$Setup="dist/$version/Oracle-$version-Setup.exe"}
$output=Join-Path $root ('.test/setup-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $output|Out-Null
$exe=Join-Path $output 'check.exe'
$csc=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
& $csc '/nologo' '/target:exe' '/r:System.Windows.Forms.dll' '/r:System.Drawing.dll' "/out:$exe" (Join-Path $root 'tests/installer-ui.cs')
if($LASTEXITCODE){throw 'Installer UI check compile failed'}
& $exe (Join-Path $root $Setup) $output
if($LASTEXITCODE){throw 'Installer UI check failed'}
