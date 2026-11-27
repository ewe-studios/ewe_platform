# Run this script only in the isolated hosted Windows storage preview.
param(
    [Parameter(Mandatory)]
    [string] $SourceRevision
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$probeRoot = Join-Path $env:RUNNER_TEMP ("ewe-sql-" + [Guid]::NewGuid().ToString('N').Substring(0, 6))
New-Item -ItemType Directory -Path $probeRoot | Out-Null
$probeRoot = (Get-Item -LiteralPath $probeRoot).FullName
if ($probeRoot -notmatch '^[A-Za-z]:\\') {
    throw 'The Windows path probe needs a local drive path.'
}

$binding = Join-Path $probeRoot 'bindgen.rs'
$mixed = Join-Path $probeRoot 'mixed.rs'
$joined = Join-Path $probeRoot 'joined.rs'
Set-Content -LiteralPath $binding -Value 'pub const BINDING: bool = true;'
Set-Content -LiteralPath $mixed -Value 'include!(concat!(env!("EWE_WINDOWS_PROBE_OUT"), "/bindgen.rs"));'
Set-Content -LiteralPath $joined -Value 'include!(env!("EWE_WINDOWS_PROBE_JOINED"));'

$env:EWE_WINDOWS_PROBE_OUT = '\\?\' + $probeRoot
$env:EWE_WINDOWS_PROBE_JOINED = '\\?\' + $binding
Write-Output "Native-path binding exists: $(Test-Path -LiteralPath $binding -PathType Leaf)"
rustc --crate-type lib --emit metadata -o (Join-Path $probeRoot 'mixed.rmeta') $mixed
$mixedStatus = $LASTEXITCODE
rustc --crate-type lib --emit metadata -o (Join-Path $probeRoot 'joined.rmeta') $joined
$joinedStatus = $LASTEXITCODE
$env:EWE_WINDOWS_PROBE_OUT = $probeRoot
rustc --crate-type lib --emit metadata -o (Join-Path $probeRoot 'normal.rmeta') $mixed
$normalStatus = $LASTEXITCODE
Write-Output "Synthetic include statuses mixed-verbatim=$mixedStatus joined-verbatim=$joinedStatus ordinary=$normalStatus"
if ($joinedStatus -ne 0 -or $normalStatus -ne 0) {
    throw 'Rust could not load the native or ordinary control binding.'
}

$scratch = Join-Path $probeRoot 'src'
git worktree add --detach "$scratch" "$SourceRevision"
if ($LASTEXITCODE -ne 0) {
    throw 'The scratch checkout failed.'
}
Push-Location -LiteralPath $scratch
try {
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    cargo test --locked --profile uat -p foundation_db --features libsql-local --test libsql_batch_tests imports_keep_receipts_and_conditional_updates_preserve_deletions -- --nocapture
    $status = $LASTEXITCODE
    $buildDirectory = Join-Path $scratch 'target/uat/build'
    $bindings = @()
    if (Test-Path -LiteralPath $buildDirectory -PathType Container) {
        $bindings = @(Get-ChildItem -LiteralPath $buildDirectory -Recurse -File -Filter 'bindgen.rs' |
            Where-Object FullName -Like '*\libsql-ffi\*')
    }
    Write-Output "Actual local libSQL binding files: $($bindings.Count); ordinary Cargo exit: $status"
    if ($status -ne 0) {
        exit $status
    }
}
finally {
    Pop-Location
}
