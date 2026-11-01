<#
  NEXUS load and network check.
  The file is deliberately ASCII-only, so it works in any PowerShell / Windows code page.

  Usage (PowerShell, no administrator rights needed):
      powershell -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -Seconds 60

  Measures NEXUS.exe and ALL its child processes (msedgewebview2.exe is the window engine).
  Network connections are sampled once per second for the whole measurement.
#>
param([int]$Seconds = 60)

function Get-NexusIds {
    $procs = Get-CimInstance Win32_Process
    $root = $procs | Where-Object { $_.Name -ieq 'NEXUS.exe' } | Select-Object -First 1
    if (-not $root) { return @() }
    $ids = New-Object System.Collections.Generic.HashSet[int]
    [void]$ids.Add([int]$root.ProcessId)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($p in $procs) {
            if ($ids.Contains([int]$p.ParentProcessId) -and -not $ids.Contains([int]$p.ProcessId)) {
                [void]$ids.Add([int]$p.ProcessId); $changed = $true
            }
        }
    }
    return @($ids)
}

function Get-Snapshot($ids) {
    $rows = foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) { [pscustomobject]@{ Id = [int]$id; Name = $p.ProcessName; Cpu = [double]$p.CPU; Private = [double]$p.PrivateMemorySize64 } }
    }
    return @($rows)
}

function Test-Local([string]$addr) {
    return ([string]::IsNullOrEmpty($addr) -or $addr -eq '::' -or $addr -eq '::1' -or $addr -eq '0.0.0.0' -or
            $addr -like '127.*' -or $addr -like '::ffff:127.*')
}

$ids = @(Get-NexusIds)
if ($ids.Count -eq 0) { Write-Host 'NEXUS.exe is not running. Start it and run this script again.'; exit 1 }

Write-Host ("Processes in the NEXUS tree: {0}. Measuring for {1} s - leave the computer alone." -f $ids.Count, $Seconds)
$first = Get-Snapshot $ids
$seen = @{}
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$tick = 0
while ($watch.Elapsed.TotalSeconds -lt $Seconds) {
    if ($tick % 5 -eq 0) { $ids = @(Get-NexusIds) }
    foreach ($c in @(Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $ids -contains $_.OwningProcess })) {
        $key = 'TCP {0}:{1} -> {2}:{3} [{4}] pid {5}' -f $c.LocalAddress, $c.LocalPort, $c.RemoteAddress, $c.RemotePort, $c.State, $c.OwningProcess
        $seen[$key] = @{ Remote = [string]$c.RemoteAddress; State = [string]$c.State }
    }
    foreach ($u in @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue | Where-Object { $ids -contains $_.OwningProcess })) {
        $key = 'UDP {0}:{1} pid {2}' -f $u.LocalAddress, $u.LocalPort, $u.OwningProcess
        $seen[$key] = @{ Remote = ''; State = 'UDP' }
    }
    Start-Sleep -Seconds 1
    $tick++
}
$last = Get-Snapshot @(Get-NexusIds)

$cores = [Environment]::ProcessorCount
$cpuDelta = 0.0
foreach ($row in $last) {
    $prev = $first | Where-Object { $_.Id -eq $row.Id } | Select-Object -First 1
    if ($prev) { $cpuDelta += ($row.Cpu - $prev.Cpu) } else { $cpuDelta += $row.Cpu }
}
$cpuPct = [math]::Round($cpuDelta / $Seconds / $cores * 100, 3)
$memMb = [math]::Round((($last | Measure-Object -Property Private -Sum).Sum) / 1MB, 1)

Write-Host ''
Write-Host 'Processes (private bytes = committed memory):'
$last | Sort-Object Private -Descending | ForEach-Object { Write-Host ('  {0,-22} PID {1,-6} {2,8:N1} MB' -f $_.Name, $_.Id, ($_.Private / 1MB)) }
Write-Host ''
Write-Host ('CPU over {0} s: {1} % of the whole processor ({2} logical cores)' -f $Seconds, $cpuPct, $cores)
Write-Host ('Memory (private bytes), whole tree: {0} MB' -f $memMb)

Write-Host ''
Write-Host 'Network connections seen during the measurement (sampled every second):'
$external = @($seen.Keys | Where-Object { $v = $seen[$_]; $v.State -ne 'UDP' -and -not (Test-Local $v.Remote) } | Sort-Object)
$local = @($seen.Keys | Where-Object { $v = $seen[$_]; $v.State -eq 'UDP' -or (Test-Local $v.Remote) } | Sort-Object)
if ($seen.Count -eq 0) {
    Write-Host '  none - no TCP/UDP sockets at all.'
} else {
    Write-Host '  Local-only (loopback / unconnected) sockets:'
    if ($local.Count -eq 0) { Write-Host '    none' } else { $local | ForEach-Object { Write-Host ('    ' + $_) } }
    Write-Host '  Connections to OTHER computers:'
    if ($external.Count -eq 0) { Write-Host '    none' } else { $external | ForEach-Object { Write-Host ('    ' + $_) } }
}
Write-Host ''
if ($external.Count -eq 0) { Write-Host 'RESULT: no connections to other computers were seen.' }
else { Write-Host 'RESULT: connections to other computers were seen - see the list above.' }

$dir = Join-Path $env:APPDATA 'app.nexus.tracker'
if (Test-Path $dir) {
    Write-Host ''
    Get-ChildItem $dir -Filter 'nexus.sqlite3*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Host ('Data file: {0} ({1:N1} KB)' -f $_.FullName, ($_.Length / 1KB)) }
}
