# Installs, updates and uninstalls a TEST build of the installer over a FAKE 0.1.x
# (Electron) install, and checks the takeover. Everything it touches is its own: a temp
# folder, a test GUID, a test shortcut name, a test Run value and a test updater-cache
# name, so a real Claude Swap install on the same machine is never touched. It records
# the real install's exe hash and uninstall entry before and after, and fails if they
# changed.
#
#   pwsh packaging\windows\test-installer.ps1 -Exe <claude-swap.exe>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Exe = (Resolve-Path $Exe).Path
$tag = "cswaptest$(Get-Random -Maximum 99999)"
$guid = "$tag-0000-0000-0000-000000000000"
$root = Join-Path ([IO.Path]::GetTempPath()) $tag
$dir = Join-Path $root 'Programs\cswap'
$data = Join-Path $root 'userdata'
$shortcut = "Claude Swap $tag"
$runValue = "electron.app.Claude Swap $tag"
$cache = "$tag-updater"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$guid"
$instKey = "HKCU:\Software\$guid"
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$lnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$shortcut.lnk"
$failures = @()
# Waits for the installer itself only: Start-Process -Wait also waits for every process it
# started, and --force-run starts the app, which keeps running.
function RunWait($file, [string[]]$argv) {
    $p = Start-Process $file -ArgumentList $argv -PassThru
    if (-not $p.WaitForExit(120000)) { throw "$file $argv did not finish in 120 s" }
    $p
}
function Check($ok, $what) { if ($ok) { "  ok   $what" } else { "  FAIL $what"; $script:failures += $what } }

# The real install, to prove it is left alone.
$realExe = Join-Path $env:LOCALAPPDATA 'Programs\cswap\Claude Swap.exe'
$realKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\70c76982-575b-5e2d-80a5-0b90f967d60f'
function RealState {
    $h = if (Test-Path $realExe) { (Get-FileHash $realExe -Algorithm SHA256).Hash } else { 'none' }
    $v = (Get-ItemProperty $realKey -ErrorAction SilentlyContinue).DisplayVersion
    $r = (Get-ItemProperty $runKey -ErrorAction SilentlyContinue).'electron.app.Claude Swap'
    "$h|$v|$r|$(Test-Path (Join-Path $env:LOCALAPPDATA 'cswap-desktop-updater'))"
}
$realBefore = RealState

try {
    # ---- the fake 0.1.x install, as electron-builder left it (names measured on a real one)
    New-Item -ItemType Directory -Force "$dir\resources", "$dir\locales", $data, (Join-Path $env:LOCALAPPDATA $cache) | Out-Null
    foreach ($f in 'LICENSE.electron.txt', 'LICENSES.chromium.html', 'ffmpeg.dll', 'icudtl.dat', 'resources.pak', 'chrome_100_percent.pak', 'vk_swiftshader.dll', 'Claude Swap.exe') {
        Set-Content -Path (Join-Path $dir $f) -Value 'electron'
    }
    Set-Content "$dir\resources\app.asar" 'asar'
    Set-Content "$dir\locales\en-US.pak" 'pak'
    # If the installer ever RAN the old uninstaller, this marker would appear.
    $marker = Join-Path $root 'old-uninstaller-ran'
    Set-Content (Join-Path $dir 'Uninstall Claude Swap.exe') 'not an exe: running it fails, deleting it is expected'
    Set-Content (Join-Path $dir 'notes-the-user-left.txt') 'keep me'
    New-Item -Force $instKey | Out-Null
    Set-ItemProperty $instKey InstallLocation $dir
    New-Item -Force $uninstKey | Out-Null
    Set-ItemProperty $uninstKey DisplayName 'Claude Swap 0.1.23'
    Set-ItemProperty $uninstKey DisplayVersion '0.1.23'
    Set-ItemProperty $uninstKey UninstallString "`"$dir\Uninstall Claude Swap.exe`" /currentuser"
    # A fresh runner has no Run key at all. Never `New-Item -Force` here: on an existing
    # key that would replace it, with every login item in it.
    if (-not (Test-Path $runKey)) { New-Item $runKey | Out-Null }
    Set-ItemProperty $runKey $runValue "`"$dir\Claude Swap.exe`" --hidden"

    # ---- build the test installer
    $defines = @("/DAPP_GUID=$guid", "/DSHORTCUT=$shortcut", "/DRUN_VALUE=$runValue", "/DUPDATER_CACHE=$cache", "/DDEFAULT_DIR=$root\default-not-used")
    $out = Join-Path $root 'claude-swap-desktop-win-x64-setup.exe'
    $makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
    if (-not $makensis) { $makensis = @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1 }
    & $makensis /V2 /DVERSION=1.0.0 "/DEXE=$Exe" "/DOUTFILE=$out" @defines (Join-Path $here 'installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }

    # ---- 1. takeover, exactly as 0.1.x's electron-updater starts it (install on quit)
    "takeover (/S --updated):"
    $p = RunWait $out @('/S', '--updated')
    Check ($p.ExitCode -eq 0) "installer exit code 0 (got $($p.ExitCode))"
    Check (-not (Test-Path "$dir\LICENSE.electron.txt") -and -not (Test-Path "$dir\resources") -and -not (Test-Path "$dir\locales") -and -not (Test-Path "$dir\ffmpeg.dll")) 'Electron files removed'
    Check ((Get-FileHash "$dir\Claude Swap.exe").Hash -eq (Get-FileHash $Exe).Hash) 'Claude Swap.exe is the v1 exe'
    Check ((Get-Item "$dir\Uninstall Claude Swap.exe").Length -gt 10000) 'uninstaller replaced by v1''s'
    Check (-not (Test-Path $marker)) 'old uninstaller was not run'
    Check (Test-Path "$dir\notes-the-user-left.txt") 'files not ours are kept'
    $u = Get-ItemProperty $uninstKey
    Check ($u.DisplayVersion -eq '1.0.0' -and $u.DisplayName -eq 'Claude Swap 1.0.0') "uninstall entry taken over ($($u.DisplayName))"
    Check ($u.UninstallString -eq "`"$dir\Uninstall Claude Swap.exe`"") 'UninstallString points at v1''s uninstaller'
    Check ((Get-ItemProperty $instKey).InstallLocation -eq $dir) 'InstallLocation unchanged'
    Check ((Get-ItemProperty $runKey).$runValue -eq "`"$dir\Claude Swap.exe`" --hidden") 'login item untouched (same exe path)'
    Check (Test-Path $lnk) 'Start-menu shortcut written'
    Check (-not (Test-Path (Join-Path $env:LOCALAPPDATA $cache))) 'old updater cache removed'
    Check (-not (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq "$dir\Claude Swap.exe" })) 'not started without --force-run'

    # ---- 2. v1 -> v1 over a RUNNING copy, as v1's helper does (/S --updated --force-run)
    "update over a running copy (/S --updated --force-run):"
    $env:CSWAP_DESKTOP_USERDATA = $data
    $running = Start-Process "$dir\Claude Swap.exe" -PassThru
    Start-Sleep 3
    $p = RunWait $out @('/S', '--updated', '--force-run')
    Check ($p.ExitCode -eq 0) "installer exit code 0 (got $($p.ExitCode))"
    Check ($running.HasExited) 'the running copy was stopped'
    Start-Sleep 3
    $new = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq "$dir\Claude Swap.exe" })
    Check ($new.Count -eq 1) "--force-run started it again ($($new.Count) running)"
    $new | ForEach-Object { Stop-Process -Id $_.Id -Force }
    Start-Sleep 1

    # ---- 3. uninstall
    "uninstall (/S):"
    $p = RunWait "$dir\Uninstall Claude Swap.exe" @('/S')
    # The NSIS uninstaller copies itself to %TEMP% and returns at once; wait for the copy.
    for ($i = 0; $i -lt 40 -and (Test-Path "$dir\Claude Swap.exe"); $i++) { Start-Sleep -Milliseconds 250 }
    Start-Sleep 1
    Check (-not (Test-Path "$dir\Claude Swap.exe") -and -not (Test-Path "$dir\Uninstall Claude Swap.exe")) 'exe and uninstaller removed'
    Check (Test-Path "$dir\notes-the-user-left.txt") 'files not ours are kept'
    Check (-not (Test-Path $uninstKey) -and -not (Test-Path $instKey)) 'registry entries removed'
    Check ($null -eq (Get-ItemProperty $runKey -ErrorAction SilentlyContinue).$runValue) 'login item removed'
    Check (-not (Test-Path $lnk)) 'shortcut removed'
} finally {
    Remove-Item Env:\CSWAP_DESKTOP_USERDATA -ErrorAction SilentlyContinue
    Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($root) } | ForEach-Object { Stop-Process -Id $_.Id -Force }
    Remove-Item -Recurse -Force $uninstKey, $instKey -ErrorAction SilentlyContinue
    Remove-ItemProperty $runKey $runValue -ErrorAction SilentlyContinue
    Remove-Item -Force $lnk -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force (Join-Path $env:LOCALAPPDATA $cache) -ErrorAction SilentlyContinue
    if (-not $Keep) { Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue }
}
$realAfter = RealState
Check ($realBefore -eq $realAfter) 'a real Claude Swap install (if any) was not touched'
if ($failures.Count) { "FAILED: $($failures.Count)"; exit 1 }
'all checks passed'
