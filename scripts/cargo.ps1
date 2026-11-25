param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$env:CARGO_TARGET_DIR = Join-Path $root 'target'
Push-Location $root
try {
    if (!$CargoArgs.Count) { throw 'Pass a cargo command, such as check, test, run or build.' }
    $sub = $CargoArgs[0]
    $rest = @($CargoArgs | Select-Object -Skip 1)
    & cargo $sub --manifest-path src-tauri/Cargo.toml @rest
    if ($LASTEXITCODE) { throw "cargo $sub failed ($LASTEXITCODE)" }
} finally { Pop-Location }
