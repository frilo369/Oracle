#Requires -Version 7.0
param([string]$Setup='', [switch]$CheckOnly)
# For an owner-requested real local update only. QA must use check-update-setup.ps1.
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$installed=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'oracle/bin/oracle.exe'))
if(!(Test-Path -LiteralPath $installed)){throw 'Oracle is not installed in the standard per-user location'}
function Get-IdleLeague {
    if(Get-Process -Name 'League of Legends' -ErrorAction SilentlyContinue){throw 'Update deferred while a League game is running'}
    $clients=@(Get-Process -Name LeagueClient,LeagueClientUx -ErrorAction SilentlyContinue)
    if(!$clients.Count){return 'Offline'}
    $ux=@($clients|Where-Object {$_.ProcessName -eq 'LeagueClientUx' -and $_.Path})
    if($ux.Count -ne 1){throw 'Could not confirm the League client state; nothing was changed'}
    $lock=Join-Path (Split-Path $ux[0].Path -Parent) 'lockfile'
    $handler=$null;$http=$null;$reason='reading the local League lock'
    try {
        $stream=[IO.File]::Open($lock,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
        try {
            if($stream.Length -gt 4096){throw 'Invalid local lock size'}
            $reader=[IO.StreamReader]::new($stream)
            try{$parts=$reader.ReadToEnd().Trim().Split(':')}finally{$reader.Dispose()}
        }finally{$stream.Dispose()}
        $reason='validating the local League lock'
        if($parts.Length -ne 5 -or $parts[4] -ne 'https' -or $clients.Id -notcontains [int]$parts[1]){throw 'Invalid local client'}
        $port=[int]$parts[2];if($port -lt 1 -or $port -gt 65535){throw 'Invalid local port'}
        $handler=[Net.Http.HttpClientHandler]::new();$handler.UseProxy=$false;$handler.AllowAutoRedirect=$false
        $reason='preparing the loopback connection'
        $handler.ServerCertificateCustomValidationCallback=[Net.Http.HttpClientHandler]::DangerousAcceptAnyServerCertificateValidator
        $http=[Net.Http.HttpClient]::new($handler);$http.Timeout=[TimeSpan]::FromSeconds(3)
        # Credentials remain in memory and are used on this fixed loopback endpoint only.
        $auth=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(('riot:'+$parts[3])))
        $http.DefaultRequestHeaders.Authorization=[Net.Http.Headers.AuthenticationHeaderValue]::new('Basic',$auth)
        $reason='querying the local League phase'
        $phase=$http.GetStringAsync("https://127.0.0.1:$port/lol-gameflow/v1/gameflow-phase").GetAwaiter().GetResult()|ConvertFrom-Json
        $reason=if($phase -match '^[a-zA-Z]{1,40}$'){"League phase $phase"}else{'an unrecognized League phase'}
        if($phase -notin @('None','Lobby','EndOfGame','WaitingForStats')){throw 'Busy client'}
        return $phase
    } catch {throw "Update deferred while $reason; Oracle was left unchanged"}
    finally {if($http){$http.Dispose()}elseif($handler){$handler.Dispose()}}
}
$phase=Get-IdleLeague
$oldVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($installed).ProductVersion
if($CheckOnly){[pscustomobject]@{installed=$installed;version=$oldVersion;leaguePhase=$phase;ready=$true}|ConvertTo-Json -Compress;return}
if(!$Setup){$short=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version -replace '\.0$','';$Setup="dist/$short/Oracle-$short-Setup.exe"}
$setupPath=[IO.Path]::GetFullPath((Join-Path $root $Setup))
$allowed=[IO.Path]::GetFullPath((Join-Path $root 'dist'))+[IO.Path]::DirectorySeparatorChar
if(!$setupPath.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Local update setup must be inside this workspace dist folder'}
$hash=(Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash.ToLowerInvariant()
$sums=Join-Path (Split-Path $setupPath -Parent) 'SHA256SUMS.txt'
if([IO.File]::ReadAllText($sums).Trim() -ne "$hash  $([IO.Path]::GetFileName($setupPath))"){throw 'Local setup checksum mismatch'}
$running=@(Get-Process oracle -ErrorAction SilentlyContinue|Where-Object {$_.Path -and [IO.Path]::GetFullPath($_.Path) -ieq $installed})
if($running.Count -ne 1){throw 'Expected exactly one running installed Oracle'}
$parent=$running[0]
$settings=Join-Path $env:LOCALAPPDATA 'com.frilo369.oracle/settings.json'
$before=if(Test-Path -LiteralPath $settings){(Get-FileHash -LiteralPath $settings).Hash}else{$null}
$info=[Diagnostics.ProcessStartInfo]::new($setupPath,"/UPDATE /S /WAITPID=$($parent.Id)")
$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
[void]$info.EnvironmentVariables.Remove('ORACLE_DATA_DIR');[void]$info.EnvironmentVariables.Remove('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$installer=[Diagnostics.Process]::Start($info)
Start-Sleep -Milliseconds 500
if($installer.HasExited){throw 'Silent installer could not start; Oracle was left running'}
$installer.Refresh();if($installer.MainWindowHandle -ne [IntPtr]::Zero){throw 'The silent installer unexpectedly showed a window; Oracle was left running'}
[void](Get-IdleLeague)
$quit=[Diagnostics.ProcessStartInfo]::new($installed,'--quit');$quit.UseShellExecute=$false;$quit.CreateNoWindow=$true
[void]$quit.EnvironmentVariables.Remove('ORACLE_DATA_DIR');[void]$quit.EnvironmentVariables.Remove('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$signal=[Diagnostics.Process]::Start($quit)
if(!$signal.WaitForExit(10000) -or !$parent.WaitForExit(10000)){throw 'Oracle did not exit for the requested update'}
if(!$installer.WaitForExit(60000)){throw 'Silent update is still running; inspect Oracle-Setup.log in your temporary folder'}
if($installer.ExitCode -ne 0){throw 'Silent update failed; the installer attempted to restore/restart the previous app'}
$limit=[DateTime]::UtcNow.AddSeconds(15);$restarted=$null
do {
    $restarted=Get-Process oracle -ErrorAction SilentlyContinue|Where-Object {$_.Path -and [IO.Path]::GetFullPath($_.Path) -ieq $installed}|Select-Object -First 1
    if(!$restarted){Start-Sleep -Milliseconds 200}
}while(!$restarted -and [DateTime]::UtcNow -lt $limit)
if(!$restarted){throw 'Updated Oracle did not restart'}
$expected=(Get-FileHash -LiteralPath (Join-Path $root 'target/release/oracle.exe')).Hash
if((Get-FileHash -LiteralPath $installed).Hash -ne $expected){throw 'Installed app does not match the checked workspace build'}
$after=if(Test-Path -LiteralPath $settings){(Get-FileHash -LiteralPath $settings).Hash}else{$null}
$record=[pscustomobject]@{installed=$installed;previousVersion=$oldVersion;version=[Diagnostics.FileVersionInfo]::GetVersionInfo($installed).ProductVersion;leaguePhase=$phase;setupSha256=$hash;settingsPreserved=$before -eq $after;restarted=$true;installerWindowShown=$false;dashboardShown=$restarted.MainWindowTitle -eq 'Oracle'}
$out=Join-Path $root ('.test/applied-update-'+[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+'.json')
[IO.File]::WriteAllText($out,($record|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
$record|ConvertTo-Json -Compress
Write-Output "Local update result: $out"
