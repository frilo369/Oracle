param([Parameter(Mandatory=$true)][string]$NotesFile)
# Run only after the owner explicitly authorizes this push/release.
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if(git status --porcelain){throw 'Commit the reviewed changes before publishing'}
    $origin=git remote get-url origin
    if($LASTEXITCODE -or $origin -notmatch '^https://github\.com/frilo369/Oracle(?:\.git)?$'){throw 'Origin must be the owner-authorized Oracle repository'}
    $package=(Get-Content src-tauri/tauri.conf.json -Raw|ConvertFrom-Json).version
    $version=$package -replace '\.0$','';$tag="v$package"
    $setup=Join-Path $root "dist/$version/Oracle-$version-Setup.exe"
    $sums=Join-Path $root "dist/$version/SHA256SUMS.txt"
    $notes=(Resolve-Path -LiteralPath $NotesFile).Path
    $digest=(Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    if(([IO.File]::ReadAllText($sums)).Trim() -ne "$digest  Oracle-$version-Setup.exe"){throw 'Setup checksum does not match SHA256SUMS.txt'}
    if([Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $root "dist/$version/Oracle.exe")).ProductVersion -ne $package){throw 'Portable app version mismatch'}
    $existingJson=gh release view $tag --repo frilo369/Oracle --json databaseId,isDraft 2>$null
    $existing=if($LASTEXITCODE -eq 0){$existingJson|ConvertFrom-Json}else{$null}
    if($existing -and !$existing.isDraft){throw 'This release is already published; use a new version'}
    $commit=git rev-parse HEAD
    & git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -u origin HEAD
    if($LASTEXITCODE){throw 'Source push failed; stopped before creating a release'}
    if($existing){
        & gh release edit $tag --repo frilo369/Oracle --draft --target $commit --notes-file $notes
    }else{
        & gh release create $tag $setup $sums --repo frilo369/Oracle --draft --target $commit --title "Oracle $version" --notes-file $notes
    }
    if($LASTEXITCODE){throw 'Draft release preparation failed'}
    # Draft tags are not yet exposed by REST's releases/tags route. Resolve the ID first.
    $releaseId=gh release view $tag --repo frilo369/Oracle --json databaseId --jq '.databaseId'
    if($LASTEXITCODE){throw 'Could not find the draft release'}
    $release=gh api "repos/frilo369/Oracle/releases/$releaseId"|ConvertFrom-Json
    if($LASTEXITCODE){throw 'Could not verify the draft release'}
    if(!$release.draft -or $release.tag_name -ne $tag -or $release.target_commitish -ne $commit){throw 'Draft identity/commit mismatch; stopped before publication'}
    $asset=@($release.assets|Where-Object name -eq "Oracle-$version-Setup.exe")
    if($asset.Count -ne 1 -or $asset[0].digest -ne "sha256:$digest"){throw 'GitHub asset digest mismatch; draft remains unpublished'}
    $checksumAsset=@($release.assets|Where-Object name -eq 'SHA256SUMS.txt')
    $checksumDigest=(Get-FileHash -LiteralPath $sums -Algorithm SHA256).Hash.ToLowerInvariant()
    if($checksumAsset.Count -ne 1 -or $checksumAsset[0].digest -ne "sha256:$checksumDigest"){throw 'GitHub checksum-file digest mismatch; draft remains unpublished'}
    & gh release edit $tag --repo frilo369/Oracle --draft=false --latest
    if($LASTEXITCODE){throw 'Release publication failed'}
    Write-Output "Published https://github.com/frilo369/Oracle/releases/tag/$tag"
} finally {Pop-Location}
