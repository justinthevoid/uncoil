# Installs uncoild as a per-user logon task.
#
#   powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1 [-Exe path] [-OpenRgb] [-Elevated]
#
# -Exe defaults to the uncoild.exe next to this script (a release download), else the repository's
# target\release\uncoild.exe (a build from source).
#
# The daemon runs as you, UNELEVATED (task "uncoil", run level Limited). Installing still needs one UAC
# prompt (the script asks for it), because the binary goes to %ProgramFiles%\uncoil, where only
# administrators can write, so no program running as you can swap it.
#
#   -OpenRgb   Also register "uncoil-openrgb": an elevated logon task that runs `uncoild.exe --openrgb-once`
#              for motherboard, GPU and RAM lighting through OpenRGB (RAM sits on the SMBus, which needs
#              administrator rights). What it does follows "openrgb" in %APPDATA%\uncoil\config.json:
#              "mode": "hardware" hands each device in "devices" to its own hardware mode and exits;
#              "mode": "live" starts OpenRGB as an SDK server on 127.0.0.1 (Razer detection off) and keeps
#              running while OpenRGB does, so the daemon can send it the desk effect (the task has no time
#              limit; stopping it stops that OpenRGB). OpenRGB keeps its settings in
#              %ProgramData%\uncoil\openrgb, which only administrators can change. Without -OpenRgb, an
#              existing uncoil-openrgb task is removed.
#   -Elevated  Fallback: run the daemon itself elevated ("highest privileges"), as installs before this
#              script did. Only for PCs where uncoild cannot open its devices unelevated (not seen yet). An
#              elevated daemon runs the OpenRGB hand-off or server itself and refuses to write through junctions.
param(
    [string]$Exe = $(if (Test-Path -LiteralPath "$PSScriptRoot\uncoild.exe") { "$PSScriptRoot\uncoild.exe" } else { "$PSScriptRoot\..\target\release\uncoild.exe" }),
    [string]$User = "$env:USERDOMAIN\$env:USERNAME",
    [switch]$OpenRgb,
    [switch]$Elevated
)
$ErrorActionPreference = 'Stop'

# Folders from the shell's known-folder list, not from environment variables a program could change.
$programFiles = [Environment]::GetFolderPath('ProgramFiles')
$programData = [Environment]::GetFolderPath('CommonApplicationData')
$bin = Join-Path $programFiles 'uncoil'
$log = Join-Path $bin 'install.log'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    # Installs before 0.1.0 kept the binary in your profile; it is no longer used.
    $old = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'uncoil\bin\uncoild.exe'
    if (Test-Path -LiteralPath $old) { Write-Host "An old copy from before 0.1.0 is at $old; it is no longer used, so you can delete it." }
    $src = (Resolve-Path -LiteralPath $Exe).Path
    $argList = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" -Exe `"$src`" -User `"$User`""
    if ($OpenRgb) { $argList += ' -OpenRgb' }
    if ($Elevated) { $argList += ' -Elevated' }
    $p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList $argList
    # the elevated copy writes what it did to install.log (in the admin-only folder); show it here
    if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log }
    exit $p.ExitCode
}

function Say([string]$Message) {
    Write-Host $Message
    Add-Content -LiteralPath $log -Value $Message
}

# Refuse any path whose existing parts include a junction or symbolic link: this script runs elevated, and
# a link would send its writes somewhere else.
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

# A folder only SYSTEM and Administrators can change (users may read), owned by Administrators. An existing
# folder owned by anyone else (any user may create folders in %ProgramData%) is refused, not adopted.
function New-AdminOnlyDir([string]$Path) {
    Assert-NoReparse $Path
    if (Test-Path -LiteralPath $Path) {
        $owner = (Get-Acl -LiteralPath $Path).GetOwner([Security.Principal.SecurityIdentifier]).Value
        if ($owner -ne 'S-1-5-32-544' -and $owner -ne 'S-1-5-18') {
            throw "$Path already exists and is not owned by Administrators or SYSTEM; delete it and run this again"
        }
    } else {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
    $acl = New-Object Security.AccessControl.DirectorySecurity
    $acl.SetSecurityDescriptorSddlForm('O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)')
    Set-Acl -LiteralPath $Path -AclObject $acl
}

Assert-NoReparse $bin
New-Item -ItemType Directory -Force $bin | Out-Null
Set-Content -LiteralPath $log -Value "uncoil install, $(Get-Date -Format s)"

try {
    $name = 'uncoil'
    $openrgbTask = 'uncoil-openrgb'
    $dst = Join-Path $bin 'uncoild.exe'

    # stop the running daemon (this session's only; other users' are theirs)
    foreach ($t in $name, $openrgbTask) { Stop-ScheduledTask -TaskName $t -ErrorAction SilentlyContinue }
    $session = (Get-Process -Id $PID).SessionId
    Get-Process uncoild -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $session } | Stop-Process -Force
    Start-Sleep 1

    # copy, then check the copy is byte for byte what was asked for
    $want = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash
    Copy-Item -LiteralPath $Exe -Destination $dst -Force
    $got = (Get-FileHash -LiteralPath $dst -Algorithm SHA256).Hash
    if ($got -ne $want) {
        Remove-Item -LiteralPath $dst -Force
        throw "the copied binary's SHA-256 is $got, expected $want; not installed"
    }
    Say "installed $dst (SHA-256 $got)"

    # the main task: unelevated unless -Elevated
    $runLevel = if ($Elevated) { 'Highest' } else { 'Limited' }
    $action    = New-ScheduledTaskAction -Execute $dst -WorkingDirectory $bin
    $logon     = New-ScheduledTaskTrigger -AtLogOn -User $User
    $logon.Delay = 'PT5S'
    $principal = New-ScheduledTaskPrincipal -UserId $User -LogonType Interactive -RunLevel $runLevel
    $settings  = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) `
                 -MultipleInstances IgnoreNew -RestartCount 5 -RestartInterval (New-TimeSpan -Minutes 1)
    Register-ScheduledTask -TaskName $name -Description 'uncoil lighting daemon (github.com/justinthevoid/uncoil)' `
        -Action $action -Trigger $logon -Principal $principal -Settings $settings -Force | Out-Null
    # Running the task again (e.g. a "Reapply" shortcut) should restart it fresh
    $xml = (Export-ScheduledTask -TaskName $name) -replace '<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>', '<MultipleInstancesPolicy>StopExisting</MultipleInstancesPolicy>'
    Register-ScheduledTask -TaskName $name -Xml $xml -User $User -Force | Out-Null
    Say "task '$name' registered for $User (run level $runLevel)"

    # OpenRGB's admin-only settings folder: the elevated hand-off (task or -Elevated daemon) refuses to run
    # OpenRGB without it
    if ($OpenRgb -or $Elevated) {
        New-AdminOnlyDir (Join-Path $programData 'uncoil')
        New-AdminOnlyDir (Join-Path $programData 'uncoil\openrgb')
        Say "OpenRGB settings folder: $(Join-Path $programData 'uncoil\openrgb') (administrators only)"
    }

    if ($OpenRgb) {
        $a = New-ScheduledTaskAction -Execute $dst -Argument '--openrgb-once' -WorkingDirectory $bin
        $t = New-ScheduledTaskTrigger -AtLogOn -User $User
        $t.Delay = 'PT10S'
        $p = New-ScheduledTaskPrincipal -UserId $User -LogonType Interactive -RunLevel Highest
        # no time limit: in live mode the task runs as long as OpenRGB's SDK server does
        $s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)
        Register-ScheduledTask -TaskName $openrgbTask -Description 'uncoil: motherboard, GPU and RAM lighting through OpenRGB (hand-off or live SDK server on 127.0.0.1)' `
            -Action $a -Trigger $t -Principal $p -Settings $s -Force | Out-Null
        Say "task '$openrgbTask' registered (elevated, at logon; last result 0 = done, 1 = off or nothing configured, 2 = failed)"
    } elseif (Get-ScheduledTask -TaskName $openrgbTask -ErrorAction SilentlyContinue) {
        Unregister-ScheduledTask -TaskName $openrgbTask -Confirm:$false
        Say "task '$openrgbTask' removed (install with -OpenRgb to keep it)"
    }

    Start-ScheduledTask -TaskName $name
    Say 'started'
} catch {
    Say "install failed: $_"
    exit 1
}
