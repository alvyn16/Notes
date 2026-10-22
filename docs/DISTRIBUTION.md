# Windows installation and updates

Notes 0.2.0 is a native Windows app. Setup and portable builds use the same
executable: the `Notes-Setup.exe` filename selects the native installer UI.
`Notes.exe --install` also opens setup. No browser or Node runtime is required.
Windows PowerShell is used for per-user registry/shortcut operations and for
replacement after an update; these operations run off the writing UI thread.

## Installation and removal

Setup installs to `%LOCALAPPDATA%\Programs\Notes`, writes the license/notices,
creates a Start menu shortcut and registers an Apps & Features uninstall entry.
No administrator access is requested. Close running Notes windows before
reinstalling. Setup offers optional `.md` and `.markdown` handling through a
ProgID, OpenWithProgids and RegisteredApplications capabilities. Windows retains
the user's default-app choice; Notes does not change the protected UserChoice key.
This follows [Microsoft's application registration model](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration).

Uninstall from Windows Settings or run the installed `Notes.exe --uninstall`.
It waits for its parent and refuses removal while other Notes windows are open.
It removes only known application files and its registration/shortcut. Vaults,
preferences, recovery drafts and unknown files are preserved.

## Update behavior

Settings checks `alvyn16/Notes`' latest stable GitHub release daily by default.
Disable this checkbox to stop automatic network checks. Automatic download and
installation on exit is a separate, opt-in setting. Manual Check, Download and
Install/restart controls are available. A repository with no published release
reports that no newer published release is available.

Only a newer numeric `vMAJOR.MINOR.PATCH` stable version is selected. The
`Notes.exe` asset must use this repository's HTTPS release URL, have a declared
size below 120 MB, and include a valid SHA-256 digest from the
[GitHub release asset API](https://docs.github.com/en/rest/releases/assets).
Downloads stream into a unique file beside the current executable. Incorrect
size/hash or download failure removes the staging file. This protects download
integrity; it is not a replacement for a publisher signature or repository security.

Applying waits for saved drafts and every process using that executable; it
never terminates a window. If another window remains open for five minutes,
replacement is abandoned. The old executable is retained as `Notes.exe.backup`;
a failed copy/hash check restores it. A valid signature on the installed app
requires a valid matching publisher on the update. Helper diagnostics append to
`%LOCALAPPDATA%\Notes\updates.log`. Back up the current executable and copy
`Notes.exe.backup` over it, with all Notes windows closed, to roll back manually.
Undo history is process-local; update restart does not transfer it.

Live download/restart against a published newer release is still pending;
the current repository push does not itself create a release. Local fixture
tests verify checksum rejection, replacement and backup without changing the
user's installation. Tests do not certify SmartScreen reputation or antivirus behavior.

## Build, sign and release

`./scripts/build.ps1 -Check` emits Notes.exe, Notes-Setup.exe, the portable ZIP
and SHA256SUMS.txt. `scripts/test-distribution.ps1` installs into a temporary
directory and isolated HKCU test namespace, verifies quoted file associations
and shortcuts, uninstalls while preserving a note, and tests update replacement.

For signing, use `./scripts/build.ps1 -SignThumbprint <thumbprint>` with a current
code-signing certificate and its private key in CurrentUser/My. The script rejects
expired certificates, signs with SHA-256/timestamping and verifies the result.
No certificate/private key is included in the repository. The only certificate
available during implementation was expired, so default and CI artifacts are
**unsigned**; signing support does not make them signed.

The Windows workflow tests/builds branch and PR changes and uploads artifacts.
A `v<version>` tag matching Cargo.toml additionally publishes the EXE, setup,
portable ZIP and checksums as a GitHub release. The workflow labels its artifacts
unsigned. A normal source push does not publish a release or install an update.
