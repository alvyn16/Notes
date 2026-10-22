param([switch]$Check, [string]$SignThumbprint)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    if ($Check) {
        cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed' }
        cargo test --locked
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
        cargo clippy --all-targets --locked -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Clippy failed' }
    }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    New-Item -ItemType Directory -Force -Path dist | Out-Null
    Copy-Item -LiteralPath target/release/notes.exe -Destination dist/Notes.exe -Force
    if ($SignThumbprint) { & "$PSScriptRoot/sign.ps1" -Path (Join-Path $projectRoot 'dist/Notes.exe') -CertificateThumbprint $SignThumbprint }
    Copy-Item -LiteralPath dist/Notes.exe -Destination dist/Notes-Setup.exe -Force
    Copy-Item -LiteralPath README.md,LICENSE -Destination dist -Force
    Copy-Item -LiteralPath licenses -Destination dist -Recurse -Force
    New-Item -ItemType Directory -Force -Path dist/docs | Out-Null
    Copy-Item -LiteralPath docs/VERIFICATION.md,docs/EDITOR_VERIFICATION.md,docs/PARITY_FEATURES.md,docs/DISTRIBUTION.md -Destination dist/docs -Force
    Compress-Archive -LiteralPath dist/Notes.exe,dist/README.md,dist/LICENSE,dist/docs,dist/licenses -DestinationPath dist/Notes-Windows.zip -Force
    $checksums = foreach ($name in @('Notes.exe','Notes-Setup.exe','Notes-Windows.zip')) {
        ((Get-FileHash -LiteralPath (Join-Path dist $name) -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $name)
    }
    $checksums | Set-Content -LiteralPath dist/SHA256SUMS.txt -Encoding ascii
    foreach ($legacyName in @('Writer.exe', 'Writer-Windows.zip')) {
        $legacyPath = Join-Path $projectRoot "dist/$legacyName"
        if (Test-Path -LiteralPath $legacyPath -PathType Leaf) {
            Remove-Item -LiteralPath $legacyPath
        }
    }
    Get-FileHash -LiteralPath dist/Notes.exe -Algorithm SHA256
} finally {
    Pop-Location
}
