param([Parameter(Mandatory=$true)][string]$PreviewImage)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$testRoot=[IO.Path]::GetFullPath((Join-Path $root '.test'))+'\'
$image=(Resolve-Path -LiteralPath $PreviewImage).Path
if(!$image.StartsWith($testRoot,[StringComparison]::OrdinalIgnoreCase)){throw 'Use an owned preview screenshot inside .test.'}
$output=Join-Path $testRoot ('overlay-ocr-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output|Out-Null
$helper=Join-Path $output 'capture.exe'
$compiler=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
& $compiler '/nologo' '/r:System.Drawing.dll' "/out:$helper" (Join-Path $root 'tests/overlay-capture.cs')
if($LASTEXITCODE){throw 'Bitmap helper compilation failed'}
$bitmap=Join-Path $output 'preview.bmp'
& $helper decode $image $bitmap
if($LASTEXITCODE){throw 'Preview decode failed'}
$oldBitmap=$env:ORACLE_OVERLAY_BITMAP
$oldTarget=$env:CARGO_TARGET_DIR
try{
    $env:ORACLE_OVERLAY_BITMAP=$bitmap
    $env:CARGO_TARGET_DIR=Join-Path $root 'target'
    & cargo test --manifest-path (Join-Path $root 'src-tauri/Cargo.toml') --locked --offline real_windows_ocr_reads_owned_preview '--' '--ignored'
    if($LASTEXITCODE){throw 'Windows OCR acceptance failed'}
    Write-Output "Windows OCR acceptance passed: $output"
}finally{$env:ORACLE_OVERLAY_BITMAP=$oldBitmap;$env:CARGO_TARGET_DIR=$oldTarget}
