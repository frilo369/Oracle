param([Parameter(Mandatory=$true)][string]$PreviousSetup,[string]$Setup='')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$Setup){$version=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version -replace '\.0$','';$Setup="dist/$version/Oracle-$version-Setup.exe"}
$scratch=Join-Path $root ('.test/update-setup-'+[guid]::NewGuid().ToString('N'))
$profile=Join-Path $scratch 'profile'
New-Item -ItemType Directory -Force $profile|Out-Null
$old=Join-Path $scratch 'previous.exe';$setupExe=Join-Path $scratch 'update.exe'
Copy-Item -LiteralPath (Join-Path $root $PreviousSetup) -Destination $old
Copy-Item -LiteralPath (Join-Path $root $Setup) -Destination $setupExe
function Start-Isolated([string]$File,[string]$Arguments){
    $info=[Diagnostics.ProcessStartInfo]::new($File,$Arguments)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
    $info.EnvironmentVariables['ORACLE_DATA_DIR']=$profile
    [Diagnostics.Process]::Start($info)
}
function Wait-ProcessResult($Process){if(!$Process.WaitForExit(45000)){throw 'Fixture process timed out'};$Process.ExitCode}
function Wait-Restart {
    $limit=[DateTime]::UtcNow.AddSeconds(15)
    while(!(Test-Path -LiteralPath $restartFile) -and [DateTime]::UtcNow -lt $limit){Start-Sleep -Milliseconds 100}
    if(!(Test-Path -LiteralPath $restartFile)){throw 'Installer did not restart the isolated app'}
    Get-Content -LiteralPath $restartFile -Raw|ConvertFrom-Json
}
$app=$null;$restarted=$null;$lock=$null;$setupLock=$null
try {
    if((Wait-ProcessResult (Start-Isolated $old '/S /VERIFY-PAYLOAD')) -ne 0){throw 'Previous fixture install failed'}
    $fixture=Join-Path $scratch 'fixture';$exe=Join-Path $fixture 'bin/oracle.exe';$restartFile=Join-Path $fixture 'update-restart.json'
    $settings=Join-Path $profile 'settings.json'
    [IO.File]::WriteAllText($settings,'{"guideHotkey":"F1","launchInTray":true,"trayDefault13":true,"notes":{"1":"preserve update notes"}}',[Text.UTF8Encoding]::new($false))
    $settingsHash=(Get-FileHash -LiteralPath $settings).Hash
    [IO.File]::WriteAllText((Join-Path $fixture 'bin/custom.txt'),'keep update extras')
    $before=(Get-FileHash -LiteralPath $exe).Hash
    $app=Start-Isolated $exe '--fixture --tray --skip-update-once'
    Start-Sleep -Milliseconds 500
    $setupLock=[IO.File]::Open($setupExe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $updater=Start-Isolated $setupExe "/S /VERIFY-UPDATE /WAITPID=$($app.Id)"
    Start-Sleep -Milliseconds 1200
    if($updater.HasExited -or (Get-FileHash -LiteralPath $exe).Hash -ne $before){throw 'Update did not wait for its parent'}
    if((Wait-ProcessResult (Start-Isolated $exe '--quit')) -ne 0){throw 'Could not stop fixture parent'}
    if((Wait-ProcessResult $updater) -ne 0){throw 'Fixture update failed'}
    $setupLock.Dispose();$setupLock=$null
    $result=Wait-Restart;$restarted=[Diagnostics.Process]::GetProcessById($result.pid)
    if(!$result.ok -or $restarted.HasExited){throw 'Successful update did not restart Oracle'}
    $expected=(Get-FileHash -LiteralPath (Join-Path $root 'target/release/oracle.exe')).Hash
    if((Get-FileHash -LiteralPath $exe).Hash -ne $expected){throw 'Updated app checksum mismatch'}
    if((Get-FileHash -LiteralPath $settings).Hash -ne $settingsHash -or [IO.File]::ReadAllText((Join-Path $fixture 'bin/custom.txt')) -ne 'keep update extras'){throw 'Update damaged preferences or extra files'}
    $signal=Start-Isolated $exe '--quit';[void](Wait-ProcessResult $signal);[void]$restarted.WaitForExit(10000);$restarted=$null
    Remove-Item -LiteralPath $restartFile
    if((Wait-ProcessResult (Start-Isolated $setupExe "/S /VERIFY-UPDATE /WAITPID=$PID")) -eq 0){throw 'Unrelated parent was accepted'}
    if(Test-Path -LiteralPath $restartFile){throw 'Rejected parent triggered a restart'}
    # Force a replacement failure while exercising the update-specific recovery/restart path.
    $lock=[IO.File]::Open((Join-Path $fixture 'bin/Uninstall.exe'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $app=Start-Isolated $exe '--fixture --tray --skip-update-once';Start-Sleep -Milliseconds 500
    $updater=Start-Isolated $setupExe "/S /VERIFY-UPDATE /WAITPID=$($app.Id)"
    Start-Sleep -Milliseconds 500
    $signal=Start-Isolated $exe '--quit';[void](Wait-ProcessResult $signal)
    if((Wait-ProcessResult $updater) -eq 0){throw 'Locked update unexpectedly succeeded'}
    $result=Wait-Restart;$restarted=[Diagnostics.Process]::GetProcessById($result.pid)
    if($result.ok -or $restarted.HasExited -or (Get-FileHash -LiteralPath $exe).Hash -ne $expected){throw 'Failed update did not preserve/restart the previous app'}
    [IO.File]::WriteAllText((Join-Path $scratch 'verified.json'),'{"waitedForParent":true,"upgraded":true,"restarted":true,"preservedSettings":true,"preservedExtras":true,"rejectedUnrelatedParent":true,"failedUpdatePreservedApp":true,"failedUpdateRestarted":true}')
    Write-Output "Update installer acceptance passed: $scratch"
} finally {
    if($lock){$lock.Dispose()}
    if($setupLock){$setupLock.Dispose()}
    if($restarted -and !$restarted.HasExited){$signal=Start-Isolated $exe '--quit';[void]$signal.WaitForExit(10000);[void]$restarted.WaitForExit(10000)}
    if($app -and !$app.HasExited){$signal=Start-Isolated $exe '--quit';[void]$signal.WaitForExit(10000)}
}
