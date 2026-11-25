$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    & "$PSScriptRoot/cargo.ps1" fmt -- --check
    & "$PSScriptRoot/cargo.ps1" check --locked --offline
    & node --test tests/mayhem.test.mjs tests/shortcuts.test.mjs tests/async-state.test.mjs
    if ($LASTEXITCODE) { throw 'Frontend tests failed' }
    & "$PSScriptRoot/cargo.ps1" test --locked --offline
} finally { Pop-Location }
