param(
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$InstallDir,
    [string]$Version = '0.2.0',
    [switch]$Associate,
    [switch]$Uninstall,
    [int]$ParentPid = 0,
    [switch]$TestMode,
    [string]$RegistryRoot,
    [string]$ShortcutDir
)
$ErrorActionPreference = 'Stop'
$target = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
$expected = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\Notes')).TrimEnd('\')
if ($TestMode) {
    $testBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\NotesInstallerTest-'
    if (!$target.StartsWith($testBase, [StringComparison]::OrdinalIgnoreCase)) { throw 'Test installation must stay in its temporary test directory' }
    if (!$RegistryRoot.StartsWith('HKCU:\Software\NotesInstallerTests\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid test registry scope' }
    if (![IO.Path]::GetFullPath($ShortcutDir).StartsWith((Split-Path $target) + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid test shortcut scope' }
    $classes = "$RegistryRoot\Classes"
    $uninstallKey = "$RegistryRoot\Uninstall"
} else {
    if ($target -ne $expected) { throw 'Installation must stay inside the per-user Notes application directory' }
    $RegistryRoot = 'HKCU:\Software'
    $classes = 'HKCU:\Software\Classes'
    $uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Notes'
    $ShortcutDir = Join-Path ([Environment]::GetFolderPath('Programs')) 'Notes'
}
$exe = Join-Path $target 'Notes.exe'
$progId = 'Notes.Markdown'
function Set-RegistryString([string]$Path, [string]$Name, [string]$Value) {
    New-Item -Path $Path -Force | Out-Null
    if ($Name -eq '(default)') { Set-Item -LiteralPath $Path -Value $Value; return }
    New-ItemProperty -LiteralPath $Path -Name $Name -Value $Value -PropertyType String -Force | Out-Null
}
function Notify-Associations {
    if ($TestMode) { return }
    Add-Type -Namespace NotesSetup -Name Shell -MemberDefinition '[System.Runtime.InteropServices.DllImport("shell32.dll")] public static extern void SHChangeNotify(uint events, uint flags, System.IntPtr item1, System.IntPtr item2);'
    [NotesSetup.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
}
if ($Uninstall) {
    if ($ParentPid -gt 0) {
        $parent = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
        if ($parent -and !$parent.WaitForExit(60000)) { throw 'Notes is still running; close it and try again' }
    }
    $otherWindows = @(Get-Process notes -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe })
    if ($otherWindows.Count -gt 0) {
        if (!$TestMode) {
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.MessageBox]::Show('Close the other Notes windows, then run Uninstall again.', 'Notes is still open') | Out-Null
        }
        throw 'Other Notes windows are still running; nothing was uninstalled'
    }
    if (Test-Path -LiteralPath $exe -PathType Leaf) { Remove-Item -LiteralPath $exe -Force }
    foreach ($extension in @('.md', '.markdown')) {
        $key = "$classes\$extension\OpenWithProgids"
        if (Test-Path -LiteralPath $key) { Remove-ItemProperty -LiteralPath $key -Name $progId -ErrorAction SilentlyContinue }
    }
    foreach ($key in @("$classes\$progId", "$classes\Applications\Notes.exe", $uninstallKey, "$RegistryRoot\Notes\Capabilities")) {
        if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key -Recurse -Force }
    }
    if (Test-Path -LiteralPath "$RegistryRoot\RegisteredApplications") {
        Remove-ItemProperty -LiteralPath "$RegistryRoot\RegisteredApplications" -Name 'Notes' -ErrorAction SilentlyContinue
    }
    foreach ($name in @('Notes.exe','LICENSE','THIRD-PARTY-NOTICES.txt','install.json','Notes.exe.backup')) {
        $file = Join-Path $target $name
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force }
    }
    $licenses = Join-Path $target 'licenses'
    foreach ($name in @('LaTeX-Rust-NOTICE.txt','LaTeX-Rust-MIT.txt','STIX-OFL.txt','LPPL-1.3c.txt','dvipsnames.tsv')) {
        $file = Join-Path $licenses $name
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force }
    }
    $shortcut = Join-Path $ShortcutDir 'Notes.lnk'
    if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut -Force }
    foreach ($folder in @($ShortcutDir, $licenses, $target)) {
        if ((Test-Path -LiteralPath $folder) -and !(Get-ChildItem -LiteralPath $folder -Force | Select-Object -First 1)) { Remove-Item -LiteralPath $folder }
    }
    Notify-Associations
    return
}
New-Item -ItemType Directory -Path $target -Force | Out-Null
if ([IO.Path]::GetFullPath($Source) -ne $exe) {
    Copy-Item -LiteralPath $Source -Destination (Join-Path $target 'Notes.installing.exe') -Force
    # Copy first; a running/locked target fails before registration is changed.
    Move-Item -LiteralPath (Join-Path $target 'Notes.installing.exe') -Destination $exe -Force
}
Set-RegistryString $uninstallKey 'DisplayName' 'Notes'
Set-RegistryString $uninstallKey 'DisplayVersion' $Version
Set-RegistryString $uninstallKey 'Publisher' 'alvyn16'
Set-RegistryString $uninstallKey 'InstallLocation' $target
Set-RegistryString $uninstallKey 'DisplayIcon' ('"' + $exe + '",0')
Set-RegistryString $uninstallKey 'UninstallString' ('"' + $exe + '" --uninstall')
New-Item -ItemType Directory -Path $ShortcutDir -Force | Out-Null
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $ShortcutDir 'Notes.lnk'))
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $target
$shortcut.IconLocation = "$exe,0"
$shortcut.Save()
if ($Associate) {
    Set-RegistryString "$classes\$progId" '(default)' 'Markdown note'
    Set-RegistryString "$classes\$progId\DefaultIcon" '(default)' ('"' + $exe + '",0')
    Set-RegistryString "$classes\$progId\shell\open\command" '(default)' ('"' + $exe + '" "%1"')
    Set-RegistryString "$classes\Applications\Notes.exe\shell\open\command" '(default)' ('"' + $exe + '" "%1"')
    foreach ($extension in @('.md','.markdown')) {
        Set-RegistryString "$classes\$extension\OpenWithProgids" $progId ''
        Set-RegistryString "$RegistryRoot\Notes\Capabilities\FileAssociations" $extension $progId
    }
    Set-RegistryString "$RegistryRoot\Notes\Capabilities" 'ApplicationName' 'Notes'
    Set-RegistryString "$RegistryRoot\Notes\Capabilities" 'ApplicationDescription' 'A native Markdown writing app'
    Set-RegistryString "$RegistryRoot\RegisteredApplications" 'Notes' 'Software\Notes\Capabilities'
}
Notify-Associations
@{ version=$Version; executable=$exe; associations=[bool]$Associate } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $target 'install.json') -Encoding UTF8
