# End to end, on Windows: an installed (test) copy finds a newer release on a FAKE GitHub
# (node on 127.0.0.1), downloads it, verifies its ed25519 signature, and on quit hands it
# to the helper, which waits for the exit, verifies again and runs the installer silently.
# A second round serves a release whose installer does not match its signature: it must
# never run.
#
# Needs a TRIAL build: the app with `--features stub-driver` (until the driver is merged)
# built with CSWAP_UPDATE_PUBLIC_KEY set to the TEST key's public half, and cswap-release:
#
#   $env:CSWAP_UPDATE_PUBLIC_KEY = '6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw='
#   cargo build --release -p cswap-desktop --features stub-driver -p cswap-update
#   pwsh packaging\windows\test-update.ps1 -Exe target\release\claude-swap.exe
#
# Test GUID, shortcut, Run value and folders as test-installer.ps1: a real install is
# never touched (checked at the end). It opens the app's window briefly (to close it).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Tool = 'target\release\cswap-release.exe'
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$Exe = (Resolve-Path $Exe).Path
$Tool = (Resolve-Path $Tool).Path
# The TEST key: seed = 32 bytes of 7 (crates/update/src/sign.rs tests). Never a release key.
$testSeed = 'BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc='
$tag = "cswaptest$(Get-Random -Maximum 99999)"
$guid = "$tag-0000-0000-0000-000000000000"
$root = Join-Path ([IO.Path]::GetTempPath()) $tag
$dir = Join-Path $root 'Programs\cswap'
$data = Join-Path $root 'userdata'
$srv = Join-Path $root 'server'
$shortcut = "Claude Swap $tag"
$runValue = "electron.app.Claude Swap $tag"
$cache = "$tag-updater"
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$guid"
$instKey = "HKCU:\Software\$guid"
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$lnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$shortcut.lnk"
$failures = @()
function Check($ok, $what) { if ($ok) { "  ok   $what" } else { "  FAIL $what"; $script:failures += $what } }
$realKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\70c76982-575b-5e2d-80a5-0b90f967d60f'
$realExe = Join-Path $env:LOCALAPPDATA 'Programs\cswap\Claude Swap.exe'
function RealState { "$(if (Test-Path $realExe) { (Get-FileHash $realExe).Hash })|$((Get-ItemProperty $realKey -ErrorAction SilentlyContinue).DisplayVersion)" }
$realBefore = RealState
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);' -ErrorAction SilentlyContinue

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
if (-not $makensis) { $makensis = @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1 }
$defines = @("/DAPP_GUID=$guid", "/DSHORTCUT=$shortcut", "/DRUN_VALUE=$runValue", "/DUPDATER_CACHE=$cache", "/DDEFAULT_DIR=$dir")
function Build($version, $outFile) {
    & $makensis /V1 "/DVERSION=$version" "/DEXE=$Exe" "/DOUTFILE=$outFile" @defines (Join-Path $here 'installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
}
function WaitExit($p, $ms) { if (-not $p.WaitForExit($ms)) { throw "timed out" } }
$node = $null
$app = $null
try {
    New-Item -ItemType Directory -Force $srv, $data | Out-Null
    # The installed copy, v1.0.0.
    Build '1.0.0' (Join-Path $root 'setup-1.0.0.exe')
    $p = Start-Process (Join-Path $root 'setup-1.0.0.exe') -ArgumentList '/S' -PassThru; WaitExit $p 120000
    Check (Test-Path "$dir\Claude Swap.exe") 'v1.0.0 installed in the test folder'

    # The release on the fake GitHub: v9.9.9, signed with the test key.
    $asset = 'claude-swap-desktop-win-x64-setup.exe'
    Build '9.9.9' (Join-Path $srv $asset)
    $env:CSWAP_UPDATE_SIGNING_KEY = $testSeed
    & $Tool sign --version 9.9.9 (Join-Path $srv $asset) | Out-Null
    Remove-Item Env:\CSWAP_UPDATE_SIGNING_KEY
    $port = Get-Random -Minimum 40000 -Maximum 49000
    $js = @"
const http = require('http'), fs = require('fs'), path = require('path');
const dir = process.argv[2], port = +process.argv[3], base = 'http://127.0.0.1:' + port;
http.createServer((req, res) => {
  if (req.url === '/repos/TheSkinnyRat/cswap-desktop/releases/latest') {
    const assets = fs.readdirSync(dir).filter(f => !f.endsWith('.js')).map(n => ({ name: n, browser_download_url: base + '/dl/' + n, size: fs.statSync(path.join(dir, n)).size }));
    res.writeHead(200, { 'content-type': 'application/json' });
    return res.end(JSON.stringify({ tag_name: 'v9.9.9', html_url: base + '/rel', assets }));
  }
  const f = path.join(dir, decodeURIComponent(req.url.replace('/dl/', '')));
  if (req.url.startsWith('/dl/') && fs.existsSync(f)) { res.writeHead(200); return fs.createReadStream(f).pipe(res); }
  res.writeHead(404); res.end('{}');
}).listen(port, '127.0.0.1');
"@
    Set-Content (Join-Path $srv 'server.js') $js
    $node = Start-Process node -ArgumentList (Join-Path $srv 'server.js'), $srv, $port -PassThru -WindowStyle Hidden
    Start-Sleep 1

    # closeToTray off, so closing the window quits (and installs on quit).
    Set-Content (Join-Path $data 'settings.json') '{"closeToTray": false, "autoUpdate": true}'
    $env:CSWAP_DESKTOP_USERDATA = $data
    $env:CSWAP_DESKTOP_UPDATE_API = "http://127.0.0.1:$port"
    "round 1: a signed update"
    $app = Start-Process "$dir\Claude Swap.exe" -PassThru
    $log = Join-Path $data 'updater.log'
    for ($i = 0; $i -lt 60 -and -not ((Test-Path $log) -and (Select-String -Quiet -Path $log -Pattern 'downloaded and verified')); $i++) { Start-Sleep 1 }
    Check ((Test-Path $log) -and (Select-String -Quiet -Path $log -Pattern 'downloaded and verified: 9.9.9')) 'checked after 15 s, downloaded and verified 9.9.9'
    $app.Refresh()
    [W.U]::PostMessage($app.MainWindowHandle, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    WaitExit $app 20000
    for ($i = 0; $i -lt 60 -and (Get-ItemProperty $uninstKey).DisplayVersion -ne '9.9.9'; $i++) { Start-Sleep 1 }
    Check ((Get-ItemProperty $uninstKey).DisplayVersion -eq '9.9.9') 'quitting ran the helper, which ran the 9.9.9 installer silently'
    # The entry is written mid-install; the helper logs once the installer has exited.
    for ($i = 0; $i -lt 60 -and -not (Select-String -Quiet -Path $log -Pattern '\[helper\] nsis: installed 9.9.9'); $i++) { Start-Sleep 1 }
    Check ((Select-String -Quiet -Path $log -Pattern '\[helper\] nsis: installed 9.9.9')) 'helper logged the install'
    Check (-not (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq "$dir\Claude Swap.exe" })) 'install on quit does not restart the app (as 0.1.x)'

    "round 2: an installer that does not match its signature"
    Stop-Process -Id $node.Id -Force
    Build '9.9.10' (Join-Path $srv $asset)  # different bytes, old 9.9.9 signature kept
    $js2 = $js.Replace("v9.9.9", "v9.9.10")
    Set-Content (Join-Path $srv 'server.js') $js2
    $node = Start-Process node -ArgumentList (Join-Path $srv 'server.js'), $srv, $port -PassThru -WindowStyle Hidden
    Start-Sleep 1
    Remove-Item $log
    $app = Start-Process "$dir\Claude Swap.exe" -PassThru
    for ($i = 0; $i -lt 60 -and -not ((Test-Path $log) -and (Select-String -Quiet -Path $log -Pattern '\[error\]')); $i++) { Start-Sleep 1 }
    Check ((Test-Path $log) -and (Select-String -Quiet -Path $log -Pattern 'does not carry a valid signature')) 'refused: not a valid signature'
    Check (-not (Test-Path (Join-Path $data "updates\$asset"))) 'the download was deleted'
    $app.Refresh()
    [W.U]::PostMessage($app.MainWindowHandle, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    WaitExit $app 20000
    Start-Sleep 5
    Check ((Get-ItemProperty $uninstKey).DisplayVersion -eq '9.9.9') 'nothing was installed'
} finally {
    Remove-Item Env:\CSWAP_DESKTOP_USERDATA, Env:\CSWAP_DESKTOP_UPDATE_API -ErrorAction SilentlyContinue
    if ($node) { Stop-Process -Id $node.Id -Force -ErrorAction SilentlyContinue }
    Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($root) } | ForEach-Object { Stop-Process -Id $_.Id -Force }
    if (Test-Path "$dir\Uninstall Claude Swap.exe") {
        $p = Start-Process "$dir\Uninstall Claude Swap.exe" -ArgumentList '/S' -PassThru; $null = $p.WaitForExit(60000); Start-Sleep 3
    }
    Remove-Item -Recurse -Force $uninstKey, $instKey -ErrorAction SilentlyContinue
    Remove-ItemProperty $runKey $runValue -ErrorAction SilentlyContinue
    Remove-Item -Force $lnk -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force (Join-Path $env:LOCALAPPDATA $cache) -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}
Check ((RealState) -eq $realBefore) 'a real Claude Swap install (if any) was not touched'
if ($failures.Count) { "FAILED: $($failures.Count)"; exit 1 }
'all checks passed'
