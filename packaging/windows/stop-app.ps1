# Stops every Claude Swap running from one install folder, before the installer overwrites
# it or the uninstaller removes it. Matching is on the exact exe path, then by PID: never
# by image name alone, which would also take a copy that has nothing to do with this
# folder (a portable exe, a test build). 0.1.x (Electron) runs several processes from the
# same exe (renderer, GPU, crashpad); they all match and all go.
param(
    [Parameter(Mandatory = $true)][string]$Dir,
    [int]$GraceSeconds = 10
)
$ErrorActionPreference = 'SilentlyContinue'
$target = [IO.Path]::GetFullPath((Join-Path $Dir 'Claude Swap.exe')).ToLowerInvariant()

function Get-App {
    @(Get-CimInstance Win32_Process -Filter "Name = 'Claude Swap.exe'" |
        Where-Object { $_.ExecutablePath -and $_.ExecutablePath.ToLowerInvariant() -eq $target })
}

# An updating copy is already on its way out (electron-updater quits right after starting
# the installer); give it the moment to finish before anything is forced.
$deadline = (Get-Date).AddSeconds($GraceSeconds)
while ((Get-App).Count -gt 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }

for ($round = 0; $round -lt 3; $round++) {
    $procs = Get-App
    if ($procs.Count -eq 0) { break }
    foreach ($p in $procs) {
        Write-Output "stopping pid $($p.ProcessId)"
        Stop-Process -Id $p.ProcessId -Force
    }
    Start-Sleep -Milliseconds 700
}
$left = Get-App
if ($left.Count -gt 0) {
    Write-Output "still running: $(($left | ForEach-Object ProcessId) -join ', ')"
    exit 1
}
exit 0
