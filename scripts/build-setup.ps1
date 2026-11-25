param([switch]$SkipBuild)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (!$SkipBuild) { & "$PSScriptRoot/build.ps1" }
    $package=(Get-Content src-tauri/tauri.conf.json -Raw|ConvertFrom-Json).version
    $version=$package -replace '\.0$',''
    $outDir=Join-Path $root "dist/$version"
    $out=Join-Path $outDir "Oracle-$version-Setup.exe"
    if (Test-Path -LiteralPath $out) { throw 'This setup version already exists. Increment the app version before another build.' }
    New-Item -ItemType Directory -Force $outDir|Out-Null
    $exe=Join-Path $root 'target/release/oracle.exe'
    if ([Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion -ne $package) { throw 'Build version mismatch' }
    $stage=Join-Path $root ('.test/setup-'+[guid]::NewGuid().ToString('N'))
    $payload=Join-Path $stage 'payload'
    New-Item -ItemType Directory -Force (Join-Path $payload 'bin'),(Join-Path $payload 'setup')|Out-Null
    $runtime=Join-Path $root 'dependencies/MicrosoftEdgeWebview2Setup.exe'
    if (!(Test-Path -LiteralPath $runtime)) { throw 'Place the official signed WebView2 bootstrapper in dependencies/MicrosoftEdgeWebview2Setup.exe.' }
    $signature=Get-AuthenticodeSignature -LiteralPath $runtime
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') { throw 'WebView2 bootstrapper must have a valid Microsoft signature.' }
    Copy-Item -LiteralPath $runtime -Destination (Join-Path $payload 'setup/MicrosoftEdgeWebview2Setup.exe')
    Add-Type -AssemblyName PresentationCore,WindowsBase
    $decoder=[Windows.Media.Imaging.BitmapDecoder]::Create([uri](Join-Path $root 'src/ui/backgrounds/eclipse.webp'),[Windows.Media.Imaging.BitmapCreateOptions]::PreservePixelFormat,[Windows.Media.Imaging.BitmapCacheOption]::OnLoad)
    $encoder=[Windows.Media.Imaging.JpegBitmapEncoder]::new();$encoder.QualityLevel=92;$encoder.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($decoder.Frames[0]))
    $wallpaper=Join-Path $stage 'wallpaper.jpg';$stream=[IO.File]::Create($wallpaper);try{$encoder.Save($stream)}finally{$stream.Dispose()}
    $buildInfo=Join-Path $stage 'BuildInfo.cs'
    $code=@"
using System.Reflection;
[assembly: AssemblyTitle("Oracle Setup")]
[assembly: AssemblyProduct("Oracle")]
[assembly: AssemblyCompany("FrYiLo")]
[assembly: AssemblyVersion("$package.0")]
[assembly: AssemblyFileVersion("$package.0")]
namespace Oracle.Installer { static class BuildInfo { public const string Version = "$version"; public const string FileVersion = "$package.0"; } }
"@
    [IO.File]::WriteAllText($buildInfo,$code)
    $csc=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    $icon=Join-Path $root 'src-tauri/icons/icon.ico'
    $common=@('/nologo','/target:winexe','/platform:anycpu','/optimize+','/warn:4','/reference:System.dll','/reference:System.Core.dll','/reference:System.Drawing.dll','/reference:System.Windows.Forms.dll','/reference:System.IO.Compression.dll','/reference:System.IO.Compression.FileSystem.dll','/reference:System.Web.Extensions.dll',"/win32icon:$icon",("/win32manifest:"+(Join-Path $root 'installer/winforms/app.manifest')),"/resource:$wallpaper,Oracle.wallpaper.jpg",("/resource:"+(Join-Path $root 'installer/wordmark.png')+',Oracle.logo.png'),"/resource:$icon,Oracle.icon.ico")
    $sources=@((Join-Path $root 'installer/winforms/OracleSetup.cs'),(Join-Path $root 'installer/winforms/SetupEngine.cs'),$buildInfo)
    & $csc @common '/define:UNINSTALLER' ("/out:"+(Join-Path $payload 'bin/Uninstall.exe')) @sources
    if ($LASTEXITCODE) { throw 'Uninstaller compilation failed' }
    Copy-Item -LiteralPath $exe -Destination (Join-Path $payload 'bin/oracle.exe')
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip=Join-Path $stage 'payload.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($payload,$zip,[IO.Compression.CompressionLevel]::Optimal,$false)
    $manifest=Join-Path $stage 'payload.sha256'
    $lines=foreach($file in Get-ChildItem -LiteralPath $payload -File -Recurse|Sort-Object FullName){"{0}`t{1}`t{2}" -f (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),$file.Length,$file.FullName.Substring($payload.Length+1).Replace('\','/')}
    [IO.File]::WriteAllLines($manifest,[string[]]$lines,[Text.UTF8Encoding]::new($false))
    & $csc @common "/out:$out" "/resource:$zip,Oracle.payload.zip" "/resource:$manifest,Oracle.payload.sha256" @sources
    if ($LASTEXITCODE) { throw 'Setup compilation failed' }
    $hash=(Get-FileHash -LiteralPath $out -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $outDir 'SHA256SUMS.txt'),"$hash  Oracle-$version-Setup.exe`n")
    Write-Output "Oracle $version setup: $out"
} finally { Pop-Location }
