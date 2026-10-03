# Installs uncoild as a per-user logon task (elevated, so OpenRGB can reach RAM over SMBus).
# Usage (from an elevated prompt or it self-elevates):  powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1 [-Exe path]
param([string]$Exe = "$PSScriptRoot\..\target\release\uncoild.exe", [string]$User = "$env:USERDOMAIN\$env:USERNAME")

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" -Exe `"$((Resolve-Path $Exe).Path)`" -User `"$User`""
    exit
}

$bin = Join-Path $env:LOCALAPPDATA 'uncoil\bin'
New-Item -ItemType Directory -Force $bin | Out-Null
$name = 'uncoil'
Stop-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
Get-Process uncoild -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
Copy-Item $Exe (Join-Path $bin 'uncoild.exe') -Force
if (Test-Path (Join-Path $old 'uncoild.exe')) { Remove-Item (Join-Path $old 'uncoild.exe') -Force }  # from installs before 0.1.0

$action    = New-ScheduledTaskAction -Execute (Join-Path $bin 'uncoild.exe') -WorkingDirectory $bin
$logon     = New-ScheduledTaskTrigger -AtLogOn -User $User
$logon.Delay = 'PT5S'
$principal = New-ScheduledTaskPrincipal -UserId $User -LogonType Interactive -RunLevel Highest
$settings  = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) `
             -MultipleInstances IgnoreNew -RestartCount 5 -RestartInterval (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName $name -Description 'uncoil lighting daemon (github.com/justinthevoid/uncoil)' `
    -Action $action -Trigger $logon -Principal $principal -Settings $settings -Force | Out-Null
# Running the task again (e.g. a "Reapply" shortcut) should restart it fresh
$xml = (Export-ScheduledTask -TaskName $name) -replace '<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>', '<MultipleInstancesPolicy>StopExisting</MultipleInstancesPolicy>'
Register-ScheduledTask -TaskName $name -Xml $xml -User $User -Force | Out-Null
Start-ScheduledTask -TaskName $name
"installed" | Set-Content (Join-Path $env:LOCALAPPDATA 'uncoil\install.out')
