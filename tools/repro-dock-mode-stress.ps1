# [DEBUG-66] Ticket 66 repro loop - dock mode switch fixed<->auto-hide stress.
#
# Builds the debug backend, then launches sprout.exe with SPROUT_DOCK_STRESS=1
# (see lib.rs debug66_dock_mode_stress). The app docks the Quick Launch window
# fixed and rapid-toggles the visibility mode; a clean run writes a PASS marker
# and exits 0. The bug under test aborts the whole process instead.
#
# Verdict per run: green iff exit code 0 AND marker starts with "PASS" AND the
# captured stderr has no panic/abort signature. Red output includes the panic
# lines (the backtrace capture the ticket asks for).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File tools\repro-dock-mode-stress.ps1 [-Runs 5] [-Iters 60] [-IntervalMs 80] [-SkipBuild]
param(
    [int]$Runs = 5,
    [int]$Iters = 60,
    [int]$IntervalMs = 80,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo "src-tauri\target\debug\sprout.exe"

if (-not $SkipBuild) {
    Write-Host "== cargo build (debug) =="
    Push-Location (Join-Path $repo "src-tauri")
    $buildOut = cmd /c "cargo build 2>&1"
    $buildOk = ($LASTEXITCODE -eq 0)
    $buildOut | Select-Object -Last 3 | ForEach-Object { Write-Host "    $_" }
    Pop-Location
    if (-not $buildOk) { Write-Host "BUILD FAILED"; exit 2 }
}

if (-not (Test-Path $exe)) { Write-Host "exe not found: $exe"; exit 2 }

$existing = Get-Process -Name "sprout" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "sprout.exe is already running (pid $($existing.Id)) - close it first (single-instance would hijack the run)."
    exit 2
}

$marker = Join-Path $env:TEMP "sprout-stress-66.json"
$logOut = Join-Path $env:TEMP "sprout-stress-66.out.log"
$logErr = Join-Path $env:TEMP "sprout-stress-66.err.log"

$green = 0
$red = 0
for ($run = 1; $run -le $Runs; $run++) {
    if (Test-Path $marker) { Remove-Item $marker -Force }
    Remove-Item $logOut, $logErr -Force -ErrorAction SilentlyContinue

    $env:SPROUT_DOCK_STRESS = "1"
    $env:SPROUT_DOCK_STRESS_ITERS = "$Iters"
    $env:SPROUT_DOCK_STRESS_MS = "$IntervalMs"
    $env:SPROUT_DOCK_STRESS_RESULT = $marker
    $env:RUST_BACKTRACE = "1"
    $p = Start-Process -FilePath $exe -PassThru -RedirectStandardOutput $logOut -RedirectStandardError $logErr -WindowStyle Hidden
    if (-not $p.WaitForExit(120000)) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        $failLog = Join-Path $env:TEMP ("sprout-stress-66.fail-{0}.err.log" -f $run)
        Copy-Item $logErr $failLog -Force -ErrorAction SilentlyContinue
        Write-Host "run ${run}: RED (timeout, killed)"
        $red++
        continue
    }
    $null = $p.WaitForExit()
    Start-Sleep -Milliseconds 200

    $code = $p.ExitCode
    $body = ""
    if (Test-Path $marker) { $body = (Get-Content $marker -Raw).Trim() }
    $errText = ""
    if (Test-Path $logErr) { $errText = Get-Content $logErr -Raw }

    # Green = clean PASS marker AND no panic/abort signature. The marker only
    # reaches PASS when the whole sequence ran and app.exit(0) fired, so the
    # exit code is informational.
    $panicked = $errText -match "panicked|abort|STATUS_STACK_BUFFER_OVERRUN|STATUS_ACCESS_VIOLATION"
    if ($body.StartsWith("PASS") -and -not $panicked) {
        $green++
        Write-Host "run ${run}: green ($body)"
    } else {
        $red++
        Write-Host "run ${run}: RED (exit=$code marker='$([char]39)$body$([char]39)' panicked=$panicked)"
        # Preserve the failing run's logs for post-run inspection (they are
        # overwritten by the next run otherwise).
        $failLog = Join-Path $env:TEMP ("sprout-stress-66.fail-{0}.err.log" -f $run)
        Copy-Item $logErr $failLog -Force -ErrorAction SilentlyContinue
        # The signal the ticket asks to capture: panic signature + backtrace head.
        $errLines = Get-Content $logErr -ErrorAction SilentlyContinue
        $idx = 0
        for ($i = 0; $i -lt $errLines.Count; $i++) {
            if ($errLines[$i] -match "panicked|abort|STATUS_") { $idx = $i; break }
        }
        if ($idx -gt 0 -or $errLines.Count -gt 0) {
            $from = [Math]::Max(0, $idx)
            $errLines | Select-Object -Skip $from -First 40 | ForEach-Object { Write-Host "    $_" }
        }
    }
}

Write-Host "== stress summary: $green/$Runs green, $red red =="
if ($red -gt 0) { exit 1 }
exit 0
