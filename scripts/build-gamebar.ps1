param([switch]$Register)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$sdkVersion='7.3.2607010'
$sdkRoot=Join-Path $root "dependencies/gamebar-$sdkVersion"
$kitRoot=Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10'
$kit='10.0.26100.0'
$bin=Join-Path $kitRoot "bin/$kit/x64"
$work=Join-Path $root ('.test/gamebar-build-'+[guid]::NewGuid().ToString('N'))
$package=Join-Path $work 'package'
New-Item -ItemType Directory -Force $work,$package,(Join-Path $package 'Assets'),(Join-Path $package 'GameBar')|Out-Null
if(!(Test-Path -LiteralPath (Join-Path $sdkRoot 'lib/uap10.0/Microsoft.Gaming.XboxGameBar.winmd'))){
    New-Item -ItemType Directory -Force $sdkRoot|Out-Null
    $zip=Join-Path $work 'sdk.zip'
    Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.gaming.xboxgamebar/$sdkVersion/microsoft.gaming.xboxgamebar.$sdkVersion.nupkg" -OutFile $zip
    Expand-Archive -LiteralPath $zip -DestinationPath $sdkRoot -Force
}
$generated=Join-Path $work 'generated'
& "$bin/cppwinrt.exe" -input (Join-Path $sdkRoot 'lib/uap10.0/Microsoft.Gaming.XboxGameBar.winmd') -reference (Join-Path $kitRoot "UnionMetadata/$kit/Windows.winmd") -output $generated
if($LASTEXITCODE){throw 'Game Bar projections failed'}
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if(!$vs){throw 'MSVC x64 compiler is required'}
$dev=Join-Path $vs 'Common7/Tools/VsDevCmd.bat'
$source=Join-Path $root 'gamebar/OracleWidget.cpp'
$exe=Join-Path $package 'OracleWidget.exe'
$command=Join-Path $work 'compile.cmd'
[IO.File]::WriteAllText($command,"@echo off`r`ncall `"$dev`" -arch=x64 -host_arch=x64 >nul`r`nif errorlevel 1 exit /b 1`r`ncl /nologo /std:c++20 /EHsc /O2 /MT /W4 /DWIN32_LEAN_AND_MEAN /DNOMINMAX /I`"$generated`" `"$source`" /Fo`"$work/OracleWidget.obj`" /Fe`"$exe`" /link /SUBSYSTEM:WINDOWS windowsapp.lib runtimeobject.lib`r`nexit /b %errorlevel%`r`n")
& cmd.exe /d /c $command
if($LASTEXITCODE){throw 'Game Bar widget compilation failed'}
Copy-Item -LiteralPath (Join-Path $sdkRoot 'lib/uap10.0/Microsoft.Gaming.XboxGameBar.winmd'),(Join-Path $sdkRoot 'private/Microsoft.Gaming.XboxGameBar.Private.winmd'),(Join-Path $sdkRoot 'runtimes/win10-x64/native/Microsoft.Gaming.XboxGameBar.dll'),(Join-Path $sdkRoot 'runtimes/win10-x64/native/Microsoft.Gaming.XboxGameBar.pri') -Destination $package
Copy-Item -LiteralPath (Join-Path $root 'gamebar/AppxManifest.xml') -Destination $package
Add-Type -AssemblyName System.Drawing
$icon=[Drawing.Icon]::new((Join-Path $root 'src-tauri/icons/icon.ico'))
try{foreach($size in @(44,150,50)){$bmp=[Drawing.Bitmap]::new($size,$size);$g=[Drawing.Graphics]::FromImage($bmp);try{$g.DrawIcon($icon,[Drawing.Rectangle]::new(0,0,$size,$size));$bmp.Save((Join-Path $package "Assets/Logo$size.png"),[Drawing.Imaging.ImageFormat]::Png)}finally{$g.Dispose();$bmp.Dispose()}}}finally{$icon.Dispose()}
$version=(Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version
$revision=0
$existing=Get-AppxPackage Frilo369.OracleWidget -ErrorAction SilentlyContinue
if($existing -and $existing.Version.ToString().StartsWith($version+'.')){$revision=([version]$existing.Version).Revision+1}
$manifest=Join-Path $package 'AppxManifest.xml'
[IO.File]::WriteAllText($manifest,([IO.File]::ReadAllText($manifest).Replace('ORACLE_VERSION',"$version.$revision")))
Write-Output "Game Bar development package: $package"
[IO.File]::WriteAllText((Join-Path $root '.test/gamebar-latest.json'),(@{package=$package}|ConvertTo-Json))
if($Register){Add-AppxPackage -Register $manifest -ForceApplicationShutdown -ErrorAction Stop;Write-Output 'Registered Oracle Game Bar widget for the current user'}
