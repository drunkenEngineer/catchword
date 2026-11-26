# Measure Catchword's start-up and memory on this PC (PERF-1, RSC-1), for
# docs/benchmarks. Run it with indexing paused or up to date, so the memory
# is the idle memory:
#
#   powershell -ExecutionPolicy Bypass -File scripts/measure-app.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/measure-app.ps1 -App target\try-app\Catchword.exe
#
# Each run starts the app, reads from its log when the window first asked
# for the status (the window is up) and when the model was ready, waits for
# things to settle, then adds up the memory of the app and its WebView2
# processes, and closes it.
#
# -Affinity keeps the app, and the processes it starts, on some logical
# processors only: a mask in hexadecimal, F000 for processors 12 to 15. On
# a processor with efficiency cores, running on those alone stands in for a
# slower laptop (docs/benchmarks/2026-11-26-stand-in.md).
param(
    [string]$App = (Join-Path $env:LOCALAPPDATA "Catchword\Catchword.exe"),
    [int]$Runs = 3,
    [int]$SettleSeconds = 15,
    [string]$Affinity = ""
)
$ErrorActionPreference = "Stop"
$log = Join-Path $env:LOCALAPPDATA "org.catchword.desktop\logs\catchword.log"

function Get-ProcessTree([int]$Root) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, CreationDate)
    # Windows reuses process ids: an older process can name as its parent
    # an id the app has since been given. Only later processes are its own.
    $started = ($all | Where-Object { $_.ProcessId -eq $Root }).CreationDate
    $all = @($all | Where-Object { $_.CreationDate -ge $started })
    $ids = @($Root)
    $frontier = @($Root)
    while ($frontier.Count -gt 0) {
        $next = @($all | Where-Object { $frontier -contains $_.ParentProcessId } | ForEach-Object { [int]$_.ProcessId })
        $ids += $next
        $frontier = $next
    }
    Get-Process -Id $ids -ErrorAction SilentlyContinue
}

function Stop-Catchword {
    Get-Process -Name Catchword -ErrorAction SilentlyContinue | ForEach-Object {
        $_.CloseMainWindow() | Out-Null
        if (-not $_.WaitForExit(10000)) { Stop-Process -Id $_.Id -Force }
    }
}

function Wait-LogEvent([int]$Skip, [string]$Event, [int]$TimeoutSeconds) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $log) {
            foreach ($line in @(Get-Content $log | Select-Object -Skip $Skip)) {
                $entry = $line | ConvertFrom-Json
                if ($entry.event -eq $Event) { return [int]$entry.millis }
            }
        }
        Start-Sleep -Milliseconds 100
    }
    return $null
}

if (-not (Test-Path $App)) { throw "No app at $App" }
Stop-Catchword
$results = @()
for ($run = 1; $run -le $Runs; $run++) {
    $skip = if (Test-Path $log) { @(Get-Content $log).Count } else { 0 }
    $process = Start-Process -FilePath $App -PassThru
    if ($Affinity) { $process.ProcessorAffinity = [IntPtr][Convert]::ToInt64($Affinity, 16) }
    $window = Wait-LogEvent $skip "interface.ready" 30
    $model = Wait-LogEvent $skip "model.ready" 60
    Start-Sleep -Seconds $SettleSeconds
    $tree = @(Get-ProcessTree $process.Id)
    $results += [pscustomobject]@{
        Run            = $run
        "Window ms"    = $window
        "Model ms"     = $model
        Processes      = $tree.Count
        "Private MB"   = [math]::Round(($tree | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB)
        "Working set MB" = [math]::Round(($tree | Measure-Object WorkingSet64 -Sum).Sum / 1MB)
    }
    Stop-Catchword
    Start-Sleep -Seconds 2
}
$results | Format-Table -AutoSize
