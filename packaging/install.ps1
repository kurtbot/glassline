# glassline installer — Windows PowerShell 5.1+ / PowerShell 7+.
#
# Detects OS/arch, downloads the matching release archive from GitHub,
# verifies its SHA256 against the release's SHA256SUMS.txt, extracts the
# `glassline.exe` binary to $env:GLASSLINE_INSTALL_DIR
# (default: $env:LOCALAPPDATA\glassline).
#
# Usage:
#   iwr https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.ps1 -UseBasicParsing | iex
#   & ([scriptblock]::Create((iwr https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.ps1 -UseBasicParsing).Content)) -Version v0.5.0
#
# Env overrides:
#   $env:GLASSLINE_INSTALL_DIR — where to drop the binary. Default: $env:LOCALAPPDATA\glassline.
#
# PATH: by default the installer appends $Dir to the User-scope PATH
# (idempotent). Pass -NoPath to opt out; the script will print the
# equivalent [Environment]::SetEnvironmentVariable(...) call instead.

[CmdletBinding()]
param(
    [string]$Version = 'latest',
    [string]$Dir = $env:GLASSLINE_INSTALL_DIR,
    [switch]$NoPath
)

$ErrorActionPreference = 'Stop'
$Repo = 'kurtbot/glassline'

if ([string]::IsNullOrEmpty($Dir)) {
    $Dir = Join-Path $env:LOCALAPPDATA 'glassline'
}

# ---------- arch detection ----------

$arch = if ([Environment]::Is64BitOperatingSystem) {
    switch -Wildcard ($env:PROCESSOR_ARCHITECTURE) {
        'ARM64' { 'aarch64' }
        default { 'x86_64' }
    }
} else {
    Write-Error 'glassline requires a 64-bit Windows installation.'
    exit 1
}

if ($arch -eq 'aarch64') {
    Write-Error 'Windows on ARM builds are not yet published. Please use --target=x86_64 via emulation, or open an issue.'
    exit 1
}

$target = 'x86_64-pc-windows-msvc'
$archive = "glassline-$target.zip"

# ---------- resolve version ----------

if ($Version -eq 'latest') {
    Write-Host 'resolving latest release...'
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    $Version = $release.tag_name
    if ([string]::IsNullOrEmpty($Version)) {
        Write-Error 'could not resolve latest release tag'
        exit 1
    }
}

$base = "https://github.com/$Repo/releases/download/$Version"
$archiveUrl = "$base/$archive"
$sumsUrl = "$base/SHA256SUMS.txt"

Write-Host "target:  $target"
Write-Host "version: $Version"
Write-Host "archive: $archiveUrl"
Write-Host "dir:     $Dir"

# ---------- download ----------

$tmp = Join-Path $env:TEMP ("glassline-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $archivePath = Join-Path $tmp $archive
    Write-Host 'downloading archive...'
    Invoke-WebRequest -Uri $archiveUrl -OutFile $archivePath -UseBasicParsing

    Write-Host 'downloading SHA256SUMS.txt...'
    $sumsPath = Join-Path $tmp 'SHA256SUMS.txt'
    try {
        Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath -UseBasicParsing
        $expectedLine = Select-String -Path $sumsPath -Pattern "  $([regex]::Escape($archive))$" | Select-Object -First 1
        if ($null -eq $expectedLine) {
            Write-Error "no SHA256 line for $archive in SHA256SUMS.txt"
            exit 1
        }
        $expected = ($expectedLine.Line -split '\s+')[0]
        $actual = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($expected -ne $actual) {
            Write-Error "SHA256 mismatch! expected=$expected actual=$actual"
            exit 1
        }
        Write-Host 'sha256 OK'
    } catch {
        Write-Warning 'SHA256SUMS.txt not found on release. Skipping verification.'
        Write-Warning '  (releases prior to v0.5.1 do not ship the sums file yet.)'
    }

    # ---------- extract + install ----------

    Write-Host 'extracting...'
    $extractDir = Join-Path $tmp 'extract'
    Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force

    New-Item -ItemType Directory -Path $Dir -Force | Out-Null
    Write-Host ''

    # Install both the render binary and the editor. `glassline-tui.exe`
    # became part of the archive in v0.6.2; older archives ship only the
    # render binary, so a missing sibling is a warning, not fatal.
    $installed = 0
    foreach ($name in @('glassline.exe', 'glassline-tui.exe')) {
        $found = Get-ChildItem -Path $extractDir -Filter $name -Recurse -File | Select-Object -First 1
        if ($null -eq $found) {
            if ($name -eq 'glassline.exe') {
                Write-Error "$name not found in archive"
                exit 1
            } else {
                Write-Warning "$name not in this archive (pre-v0.6.2). Interactive editor will not launch."
                continue
            }
        }
        $dest = Join-Path $Dir $name
        Copy-Item -Path $found.FullName -Destination $dest -Force
        Write-Host "installed: $dest"
        $installed++
    }

    if ($installed -eq 0) {
        Write-Error 'no binaries installed'
        exit 1
    }

    $pathElements = $env:PATH -split ';'
    if ($pathElements -contains $Dir) {
        Write-Host "PATH already contains $Dir — you're set."
    } elseif ($NoPath) {
        Write-Host ''
        Write-Host "NOTE: $Dir is not on your PATH (-NoPath given)."
        Write-Host '  Add it later via:'
        Write-Host "    [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$Dir`", 'User')"
        Write-Host '  (then restart your shell)'
    } else {
        # Persist to the User-scope PATH so the change survives a shell
        # restart. Idempotent: only append if $Dir isn't already there.
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $userPathElements = if ($userPath) { $userPath -split ';' } else { @() }
        if ($userPathElements -contains $Dir) {
            Write-Host "User PATH already contains $Dir — no change written."
            Write-Host '  Restart your shell to pick it up in this session.'
        } else {
            $newUserPath = if ([string]::IsNullOrEmpty($userPath)) { $Dir } else { "$userPath;$Dir" }
            [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
            Write-Host ''
            Write-Host "added $Dir to User PATH."
            Write-Host '  Restart your shell (or open a new terminal) to pick it up.'
            Write-Host '  Pass -NoPath to future installer runs to skip this.'
        }
    }

    Write-Host ''
    Write-Host "next: run 'glassline install' to wire it into ~/.claude/settings.json"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
