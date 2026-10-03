# Removes the uncoil logon task and its binary. Settings in %APPDATA%\uncoil are kept; delete that folder too for a clean slate.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\uninstall-task.ps1   (self-elevates)
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

$name = 'uncoil'
Stop-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
Get-Process uncoild -ErrorAction SilentlyContinue | Stop-Process -Force
Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
Start-Sleep 1
foreach ($dir in (Join-Path $env:ProgramFiles 'uncoil'), (Join-Path $env:LOCALAPPDATA 'uncoil\bin')) {
    $exe = Join-Path $dir 'uncoild.exe'
    if (Test-Path $exe) { Remove-Item $exe -Force }
    if ((Test-Path $dir) -and -not (Get-ChildItem $dir)) { Remove-Item $dir -Force }
}
