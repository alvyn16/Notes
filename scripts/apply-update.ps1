param(
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$Target,
    [Parameter(Mandatory=$true)][string]$Sha256,
    [int]$ParentPid = 0,
    [string]$Version,
    [switch]$Restart,
    [switch]$TestMode
)
$ErrorActionPreference = 'Stop'
$sourcePath = [IO.Path]::GetFullPath($Source)
$targetPath = [IO.Path]::GetFullPath($Target)
if ((Split-Path $sourcePath) -ne (Split-Path $targetPath) -or (Split-Path $targetPath -Leaf) -ne 'Notes.exe') { throw 'Update must remain beside the Notes executable' }
if ((Split-Path $sourcePath -Leaf) -notmatch '^\.Notes-update-[A-Za-z0-9.-]+\.exe$') { throw 'Invalid staged update filename' }
if ($Sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw 'Invalid update checksum' }
if ($Version -and $Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid update version' }
if ($TestMode -and !$targetPath.StartsWith(([IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\NotesUpdateTest-'), [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid test update scope' }
if ((Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash -ne $Sha256) { throw 'Update checksum mismatch' }
$signature = Get-AuthenticodeSignature -LiteralPath $targetPath
if ($signature.Status -eq 'Valid') {
    $updatedSignature = Get-AuthenticodeSignature -LiteralPath $sourcePath
    if ($updatedSignature.Status -ne 'Valid' -or $updatedSignature.SignerCertificate.Subject -ne $signature.SignerCertificate.Subject) { throw 'The update publisher does not match the installed app' }
}
if ($ParentPid -gt 0) {
    $parent = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
    if ($parent -and !$parent.WaitForExit(60000)) { throw 'Notes is still running; update was not installed' }
}
# Wait for every workspace using this executable. Never terminate a draft owner.
$deadline = [DateTime]::UtcNow.AddMinutes(5)
do {
    $running = @(Get-Process notes -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $targetPath })
    if ($running.Count -eq 0) { break }
    if ([DateTime]::UtcNow -gt $deadline) { throw 'Other Notes windows are still open; update was not installed' }
    Start-Sleep -Milliseconds 300
} while ($true)
$backup = "$targetPath.backup"
Copy-Item -LiteralPath $targetPath -Destination $backup -Force
try {
    Copy-Item -LiteralPath $sourcePath -Destination $targetPath -Force
    if ((Get-FileHash -LiteralPath $targetPath -Algorithm SHA256).Hash -ne $Sha256) { throw 'Installed update verification failed' }
} catch {
    Copy-Item -LiteralPath $backup -Destination $targetPath -Force
    throw
}
Remove-Item -LiteralPath $sourcePath
if ($Version -and $targetPath -eq (Join-Path $env:LOCALAPPDATA 'Programs\Notes\Notes.exe')) {
    $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Notes'
    if (Test-Path -LiteralPath $registration) { Set-ItemProperty -LiteralPath $registration -Name DisplayVersion -Value $Version }
}
if ($Restart -and !$TestMode) { Start-Process -FilePath $targetPath -WindowStyle Normal }
