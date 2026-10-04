# Removes the uncoil logon tasks ("uncoil" and "uncoil-openrgb"), the binary in %ProgramFiles%\uncoil and
# OpenRGB's settings folder %ProgramData%\uncoil. Your settings in %APPDATA%\uncoil and the log, status and
# journal in %LOCALAPPDATA%\uncoil are kept; delete those folders yourself for a clean slate.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\uninstall-task.ps1   (asks for elevation)
$ErrorActionPreference = 'Stop'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    $old = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'uncoil\bin\uncoild.exe'
    if (Test-Path -LiteralPath $old) { Write-Host "An old copy from before 0.1.0 is at $old; delete it yourself." }
    $p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit $p.ExitCode
}

# Refuse any path whose existing parts include a junction or symbolic link (this runs elevated).
function Assert-NoReparse([string]$Path) {
    $p = [IO.Path]::GetFullPath($Path)
    while ($p) {
        if (Test-Path -LiteralPath $p) {
            $item = Get-Item -LiteralPath $p -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "$p is a junction or link; refusing to use it" }
        }
        $parent = [IO.Path]::GetDirectoryName($p)
        if ($parent -eq $p) { break }
        $p = $parent
    }
}

foreach ($name in 'uncoil', 'uncoil-openrgb') {
    Stop-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
}
$session = (Get-Process -Id $PID).SessionId
Get-Process uncoild -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $session } | Stop-Process -Force
Start-Sleep 1

$bin = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'uncoil'
if (Test-Path -LiteralPath $bin) {
    Assert-NoReparse $bin
    foreach ($f in 'uncoild.exe', 'install.log') {
        $file = Join-Path $bin $f
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
    }
    if (-not (Get-ChildItem -LiteralPath $bin -Force)) { Remove-Item -LiteralPath $bin -Force }
}

# OpenRGB's settings for the hand-off. Only a folder the installer made (owned by Administrators or SYSTEM)
# is removed, with rmdir, which deletes links inside it rather than following them.
$data = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'uncoil'
if (Test-Path -LiteralPath $data) {
    Assert-NoReparse $data
    $owner = (Get-Acl -LiteralPath $data).GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -eq 'S-1-5-32-544' -or $owner -eq 'S-1-5-18') {
        & (Join-Path ([Environment]::SystemDirectory) 'cmd.exe') /d /c rmdir /s /q "$data"
    } else {
        Write-Host "$data is not owned by Administrators or SYSTEM, so it was left alone"
    }
}
