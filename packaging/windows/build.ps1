# Builds the Windows release files into packaging\windows\out:
#
#   claude-swap-desktop-win-x64-setup.exe       NSIS installer (per-user, takes over 0.1.x)
#   claude-swap-desktop-win-x64-portable.exe    the bare exe
#   <each>.sig                                  ed25519 signatures (when the key is set)
#   latest.yml                                  electron-updater feed for 0.1.x copies
#
# Same script in release.yml and on a dev PC. The version is the root Cargo.toml's
# [workspace.package] version.
#
#   pwsh packaging\windows\build.ps1 [-SkipBuild] [-Exe <claude-swap.exe>] [-Defines @('/DAPP_GUID=test')]
#
# Signing needs CSWAP_UPDATE_SIGNING_KEY; without it the files are built unsigned (a dev
# build) unless -RequireSignature is given (release.yml).
param(
    [string]$Exe,
    [switch]$SkipBuild,
    [switch]$RequireSignature,
    [string[]]$Defines = @(),
    [string]$Makensis
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = (Resolve-Path (Join-Path $here '..\..')).Path

$toml = [IO.File]::ReadAllText((Join-Path $root 'Cargo.toml'))
$m = [regex]::Match($toml, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"')
if (-not $m.Success) { throw 'no [workspace.package] version in Cargo.toml' }
$version = $m.Groups[1].Value
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "version '$version' must be x.y.z (NSIS VIProductVersion)" }

Push-Location $root
try {
    if (-not $Exe) {
        if (-not $SkipBuild) {
            cargo build --release --locked -p cswap-desktop -p cswap-update
            if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
        }
        $Exe = Join-Path $root 'target\release\claude-swap.exe'
    }
    $Exe = (Resolve-Path $Exe).Path
    $tool = Join-Path $root 'target\release\cswap-release.exe'

    if (-not $Makensis) {
        $cmd = Get-Command makensis -ErrorAction SilentlyContinue
        $Makensis = if ($cmd) { $cmd.Source } else {
            @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
        }
    }
    if (-not $Makensis) { throw 'makensis not found (winget install NSIS.NSIS)' }

    $out = Join-Path $here 'out'
    New-Item -ItemType Directory -Force $out | Out-Null
    $setup = Join-Path $out 'claude-swap-desktop-win-x64-setup.exe'
    $portable = Join-Path $out 'claude-swap-desktop-win-x64-portable.exe'
    & $Makensis /V2 "/DVERSION=$version" "/DEXE=$Exe" "/DOUTFILE=$setup" @Defines (Join-Path $here 'installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
    Copy-Item -Force $Exe $portable

    if ($env:CSWAP_UPDATE_SIGNING_KEY) {
        & $tool sign --version $version $setup $portable
        if ($LASTEXITCODE -ne 0) { throw "signing failed ($LASTEXITCODE)" }
        # The key the app will trust (compiled in) must accept what was just signed.
        & $tool verify --version $version $setup $portable
        if ($LASTEXITCODE -ne 0) { throw "the built app's release key does not verify these signatures ($LASTEXITCODE)" }
    } elseif ($RequireSignature) {
        throw 'CSWAP_UPDATE_SIGNING_KEY is not set'
    } else {
        Write-Warning 'CSWAP_UPDATE_SIGNING_KEY not set: files are NOT signed (dev build)'
    }
    # 0.1.x -> v1 only (LEGACY-UPDATE.md).
    & $tool legacy-feed --version $version --out (Join-Path $out 'latest.yml') $setup
    if ($LASTEXITCODE -ne 0) { throw "legacy feed failed ($LASTEXITCODE)" }
} finally { Pop-Location }

Get-ChildItem $out | Format-Table Name, Length
