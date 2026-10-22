$ErrorActionPreference = 'Stop'
$id = [Guid]::NewGuid().ToString('N')
$installBase = Join-Path ([IO.Path]::GetTempPath()) ("NotesInstallerTest-" + $id)
$updateBase = Join-Path ([IO.Path]::GetTempPath()) ("NotesUpdateTest-" + $id)
$registry = "HKCU:\Software\NotesInstallerTests\$id"
$target = Join-Path $installBase 'Notes'
$shortcut = Join-Path $installBase 'Start Menu'
New-Item -ItemType Directory -Path $installBase,$updateBase -Force | Out-Null
$source = Join-Path $installBase "setup with spaces' quote.exe"
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../dist/Notes.exe') -Destination $source
try {
    & "$PSScriptRoot/install.ps1" -Source $source -InstallDir $target -Associate -TestMode -RegistryRoot $registry -ShortcutDir $shortcut
    if (!(Test-Path -LiteralPath (Join-Path $target 'Notes.exe'))) { throw 'Installed executable missing' }
    $command = (Get-Item -LiteralPath "$registry\Classes\Notes.Markdown\shell\open\command").GetValue('')
    if ($command -ne ('"' + (Join-Path $target 'Notes.exe') + '" "%1"')) { throw 'Quoted association command mismatch' }
    if (!(Test-Path -LiteralPath (Join-Path $shortcut 'Notes.lnk'))) { throw 'Start Menu shortcut missing' }
    Set-Content -LiteralPath (Join-Path $target 'user-note.md') -Value 'keep this note'
    & "$PSScriptRoot/install.ps1" -Source $source -InstallDir $target -Uninstall -TestMode -RegistryRoot $registry -ShortcutDir $shortcut
    if (Test-Path -LiteralPath (Join-Path $target 'Notes.exe')) { throw 'Uninstaller left executable' }
    if (!(Test-Path -LiteralPath (Join-Path $target 'user-note.md'))) { throw 'Uninstaller removed a user note' }
    $old = Join-Path $updateBase 'Notes.exe'
    $staged = Join-Path $updateBase '.Notes-update-fixture.exe'
    Set-Content -LiteralPath $old -Value 'old app'
    Set-Content -LiteralPath $staged -Value 'new app'
    $hash = (Get-FileHash -LiteralPath $staged -Algorithm SHA256).Hash
    $rejected = $false
    try { & "$PSScriptRoot/apply-update.ps1" -Source $staged -Target $old -Sha256 ('0' * 64) -TestMode } catch { $rejected = $true }
    if (!$rejected -or (Get-Content -LiteralPath $old -Raw).Trim() -ne 'old app') { throw 'Invalid checksum was not safely rejected' }
    & "$PSScriptRoot/apply-update.ps1" -Source $staged -Target $old -Sha256 $hash -TestMode
    if ((Get-Content -LiteralPath $old -Raw).Trim() -ne 'new app') { throw 'Update did not replace app' }
    if ((Get-Content -LiteralPath "$old.backup" -Raw).Trim() -ne 'old app') { throw 'Update backup missing' }
    Write-Output 'Installer/association/uninstall and update checksum/backup checks passed.'
} finally {
    # Verify each absolute target remains within its exact named test root.
    foreach ($path in @($installBase,$updateBase)) {
        $resolved = [IO.Path]::GetFullPath($path)
        if ($resolved -ne $installBase -and $resolved -ne $updateBase) { throw 'Unexpected cleanup path' }
        if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
    }
    if (Test-Path -LiteralPath $registry) { Remove-Item -LiteralPath $registry -Recurse -Force }
}
