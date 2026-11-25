param([string]$Setup='')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$Setup){$version=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version -replace '\.0$','';$Setup="dist/$version/Oracle-$version-Setup.exe"}
$source=Join-Path $root $Setup
$testRoot=Join-Path $root ('.test/installer-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $testRoot|Out-Null
$copy=Join-Path $testRoot 'Oracle-Setup.exe'
Copy-Item -LiteralPath $source -Destination $copy
$process=Start-Process -FilePath $copy -ArgumentList '/S','/VERIFY-PAYLOAD' -WindowStyle Hidden -PassThru -Wait
if($process.ExitCode -ne 0){throw "Fixture install failed ($($process.ExitCode)): $env:TEMP\Oracle-Setup.log"}
$fixture=Join-Path $testRoot 'fixture'
$installed=Join-Path $fixture 'bin/oracle.exe'
$expected=Get-FileHash -LiteralPath (Join-Path $root 'target/release/oracle.exe') -Algorithm SHA256
if((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $expected.Hash){throw 'Installed application checksum mismatch'}
[IO.File]::WriteAllText((Join-Path $fixture 'notebook-sentinel.txt'),'preserve this note')
New-Item -ItemType Directory -Path (Join-Path $fixture 'bin/user-files')|Out-Null
[IO.File]::WriteAllText((Join-Path $fixture 'bin/user-files/sentinel.txt'),'preserve nested files')
[IO.File]::WriteAllText((Join-Path $fixture 'bin/custom.txt'),'preserve app folder files')
$process=Start-Process -FilePath $copy -ArgumentList '/S','/VERIFY-PAYLOAD' -WindowStyle Hidden -PassThru -Wait
if($process.ExitCode -ne 0){throw 'Fixture reinstall failed'}
if([IO.File]::ReadAllText((Join-Path $fixture 'notebook-sentinel.txt')) -ne 'preserve this note'){throw 'Reinstall damaged user data'}
if([IO.File]::ReadAllText((Join-Path $fixture 'bin/user-files/sentinel.txt')) -ne 'preserve nested files' -or [IO.File]::ReadAllText((Join-Path $fixture 'bin/custom.txt')) -ne 'preserve app folder files'){throw 'Reinstall damaged application-folder data'}
$uninstall=Join-Path $fixture 'bin/Uninstall.exe'
$process=Start-Process -FilePath $uninstall -ArgumentList '/S','/VERIFY-PAYLOAD' -WindowStyle Hidden -PassThru -Wait
if($process.ExitCode -ne 0){throw 'Uninstaller bootstrap failed'}
$limit=[DateTime]::UtcNow.AddSeconds(20)
while((Test-Path -LiteralPath $installed) -and [DateTime]::UtcNow -lt $limit){Start-Sleep -Milliseconds 100}
if(Test-Path -LiteralPath $installed){throw 'Uninstaller did not remove the app'}
if([IO.File]::ReadAllText((Join-Path $fixture 'notebook-sentinel.txt')) -ne 'preserve this note'){throw 'Uninstaller damaged user data'}
if([IO.File]::ReadAllText((Join-Path $fixture 'bin/user-files/sentinel.txt')) -ne 'preserve nested files' -or [IO.File]::ReadAllText((Join-Path $fixture 'bin/custom.txt')) -ne 'preserve app folder files'){throw 'Uninstaller damaged application-folder data'}
[IO.File]::WriteAllText((Join-Path $testRoot 'verified.json'),'{"install":true,"payloadHash":true,"reinstall":true,"uninstall":true,"preservedData":true}')
Write-Output "Installer acceptance passed: $testRoot"
