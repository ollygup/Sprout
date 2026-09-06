# Repro/loop for: companion Off must leave no WebView2 behind.
#
# Symptom: with a companion site active, setting Companion to Off (Settings >
# Active site > Off, or removing the active saved site) persisted the off
# state but could leave the native `companion` WebView2 child alive — an
# orphaned renderer holding ~150-200 MB until restart.
#
# Why static: the orphan needs a real WebView2 runtime + window timing
# (in-flight creation vs. close race), so no unit seam can spawn it. This
# loop asserts the invariant at the seams that own it instead:
#   off persisted  =>  backend destroys the live child (both writers) AND
#                      the dock frontend closes + sweeps the child by label.
# Red while any half is missing; green once off truly implies no webview.
#
# Run: powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/repro-companion-off-teardown.ps1
# Exit 0 = invariant holds. Non-zero = count of violated assertions.

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$failures = 0

function Assert($name, [bool]$cond, [string]$hint) {
    if ($cond) {
        Write-Host "PASS  $name"
    } else {
        Write-Host "FAIL  $name -- $hint"
        $script:failures++
    }
}

function FnBody([string]$path, [string]$fnName) {
    $lines = Get-Content $path
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^\s*fn\s+$fnName\s*\(") { $start = $i; break }
    }
    if ($start -lt 0) { return "" }
    # Rust fns in this repo end at a column-0 closing brace.
    for ($i = $start + 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^\}") {
            return ($lines[$start..$i] -join "`n")
        }
    }
    return ($lines[$start..($lines.Count - 1)] -join "`n")
}

function SvelteFnBody([string]$path, [string]$fnName) {
    $lines = Get-Content $path
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "async function\s+$fnName\s*\(") { $start = $i; break }
    }
    if ($start -lt 0) { return "" }
    # Take a bounded window: the sync fn is self-contained within ~160 lines.
    $end = [Math]::Min($start + 160, $lines.Count - 1)
    return ($lines[$start..$end] -join "`n")
}

$libRs = Join-Path $root "src-tauri\src\lib.rs"
$audioRs = Join-Path $root "src-tauri\src\companion_audio.rs"
$qlw = Join-Path $root "src\routes\quick-launch-window\+page.svelte"
$audioSrc = Get-Content $audioRs -Raw

# 1. Backend destroy helper exists: closes the live `companion` webview by label.
Assert "backend destroy helper closes companion webview" `
    ($audioSrc -match "COMPANION_WEBVIEW_LABEL" -and $audioSrc -match "\.close\(\)") `
    "companion_audio.rs needs a destroy path via app.get_webview(COMPANION_WEBVIEW_LABEL) + close()"

# 2. set_companion_url(null) destroys the live child (single-site writer).
$setUrl = FnBody $libRs "set_companion_url"
Assert "set_companion_url off destroys live webview" `
    ($setUrl -match "destroy") `
    "set_companion_url must destroy the live companion child when the URL normalizes to off"

# 3. update_settings with companion off destroys the live child (bulk Settings writer — the actual Off-select path).
$updateSettings = FnBody $libRs "update_settings"
Assert "update_settings off destroys live webview" `
    ($updateSettings -match "destroy") `
    "update_settings must destroy the live companion child when companion_url saves as off"

# 4. Dock frontend closes the cached handle when the pane goes away (existing close path — locked in).
$syncOnce = SvelteFnBody $qlw "syncCompanionWebviewOnce"
Assert "frontend off-path closes cached webview" `
    ($syncOnce -match "companionVisible" -and $syncOnce -match "companionWebview\.close\(\)") `
    "syncCompanionWebviewOnce must close the cached handle when !companionVisible || !companionUrl"

# 5. Dock frontend sweeps a live child by label on the off-path (catches
#    in-flight creations whose close() landed before backend registration).
#    Scoped to the off-branch only (up to its early return) so the
#    create-branch dedup sweep cannot false-pass this.
$offBranch = $syncOnce
$offStart = $offBranch.IndexOf("if (!companionVisible")
if ($offStart -ge 0) {
    $offBranch = $offBranch.Substring($offStart)
    $offReturn = $offBranch.IndexOf("`n    }")
    if ($offReturn -ge 0) { $offBranch = $offBranch.Substring(0, $offReturn) }
}
Assert "frontend off-path sweeps live child by label" `
    ($offBranch -match 'getByLabel\("companion"\)') `
    "off-path must also close Webview.getByLabel('companion') so an in-flight creation cannot orphan"

# 6. Always-on RAM mitigation: the companion WebView2 is put on the LOW
#    memory usage target (healed like mute, so recreations keep it).
Assert "companion webview pinned to LOW memory target" `
    ($audioSrc -match "MemoryUsageTargetLevel") `
    "companion_audio.rs should set ICoreWebView2_19 LOW memory target on the live child"

# 7. The late orphan sweep is a bounded poll, not a single shot: a child
#    landing seconds after teardown (slow cold init on weak devices read as
#    white -> bare site -> gone at ~15-20s) must be reaped in ~1s. The poll
#    stops when the pane legitimately returns (rapid off->on) or hits its
#    hard cap, so it can never kill a new child or run forever.
Assert "frontend off-path re-sweeps on a bounded gated poll" `
    ($offBranch -match "setInterval" -and $offBranch -match "clearInterval" -and $offBranch -match "useWebview") `
    "off-path must re-sweep by label on an interval gated on useWebview with a hard stop"

# 8. Stale refresh runs discard their results: a pre-save settings read
#    landing after an off-save must not reassign the old URL (permanent
#    resurrection) or clobber a rapid off->on. Last starter wins.
$qlwSrc = Get-Content $qlw -Raw
Assert "stale companion refresh runs drop their results" `
    ($qlwSrc -match "companionRefreshGen" -and $qlwSrc -match "gen !== companionRefreshGen") `
    "refreshCompanion needs a monotonic run id with superseded runs returning before assignment"

Write-Host ""
if ($failures -eq 0) {
    Write-Host "OK: companion off implies no WebView2 (8/8)."
    exit 0
} else {
    Write-Host "$failures assertion(s) red: companion off can orphan a WebView2."
    exit $failures
}
