<#
.SYNOPSIS
  Read-only check of this machine's VM workspace assumptions.

.DESCRIPTION
  VM mode hardcodes two paths (working copy C:\Sprout, share
  \\vmware-host\Shared Folders\Projects\Sprout) because sync.ps1's snapshot
  guard only makes sense against that exact topology (docs/agents/working-copy.md).

  This script verifies the machine already matches those assumptions. It never
  creates, moves, or reparents a directory, never touches the share, and never
  writes .sprout-workspace - AGENTS.md requires the agent to show the printed
  remedy and let the human run it, then re-check.

  Exit 0: every hard check passed; the user may write `vm` and run -Down.
  Exit 1: VM mode is not available here; the printed lines say what is missing.

.EXAMPLE
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\vm-setup.ps1"
#>
param(
    # Accepted for explicitness. The script has no mutating mode, so a bare run
    # performs the same check.
    [switch]$Check
)

$ErrorActionPreference = "Stop"

$expectedLocal = "C:\Sprout"
$share = "\\vmware-host\Shared Folders\Projects\Sprout"
$root = Split-Path -Parent $PSScriptRoot
$markerPath = Join-Path $root ".sprout-workspace"

$failures = 0

function Write-Check([bool]$ok, [string]$label, [string]$detail) {
    $state = if ($ok) { "pass" } else { "FAIL" }
    if (-not $ok) { $script:failures++ }
    $suffix = if ($detail) { ": $detail" } else { "" }
    Write-Output ("  [{0}] {1}{2}" -f $state, $label, $suffix)
}

Write-Output "== VM workspace check =="
Write-Output ("  repo root: {0}" -f $root)

# 1. The working copy must be the literal path the sync script writes to.
$rootOk = ([System.IO.Path]::GetFullPath($root).TrimEnd("\") -ieq $expectedLocal)
Write-Check $rootOk "working copy is $expectedLocal" $root

if (-not $rootOk) {
    Write-Output ""
    Write-Output "  VM mode is fixed to $expectedLocal. A clone anywhere else is a standalone"
    Write-Output "  workspace and needs none of this. To keep this checkout where it is, the"
    Write-Output "  human runs:"
    Write-Output ("    New-Item -ItemType Junction -Path ""{0}"" -Target ""{1}""" -f $expectedLocal, $root)
    Write-Output "  A junction needs no elevation; builds work through it. If $expectedLocal already"
    Write-Output "  exists, inspect it first - removing a junction with Remove-Item -Recurse can"
    Write-Output "  delete the tree it points at. The safe removal is:"
    Write-Output ("    cmd /c rmdir ""{0}""" -f $expectedLocal)
    Write-Output "  Nothing here is done automatically."
}

# 2. The share is the source of truth; without it there is no VM to speak of.
$shareOk = Test-Path -LiteralPath $share
Write-Check $shareOk "share is reachable" $share

# 3. The two scripts the VM rules invoke.
$syncOk = Test-Path -LiteralPath (Join-Path $root "tools\sync.ps1") -PathType Leaf
Write-Check $syncOk "tools\sync.ps1 exists" ""
$gateOk = Test-Path -LiteralPath (Join-Path $root "tools\ownership-gate.mjs") -PathType Leaf
Write-Check $gateOk "tools\ownership-gate.mjs exists" ""

# 4. Toolchain: the share cannot host npm/cargo (.cmd helpers reject UNC paths).
foreach ($tool in @("node", "npm.cmd", "cargo")) {
    $found = Get-Command $tool -ErrorAction SilentlyContinue
    Write-Check ($null -ne $found) "toolchain: $tool on PATH" $(if ($found) { $found.Source } else { "not found" })
}

# 5. Marker state. Reported, never fixed here.
$mode = "absent"
if (Test-Path -LiteralPath $markerPath -PathType Leaf) {
    $raw = Get-Content -LiteralPath $markerPath -Raw
    $mode = switch ($raw.Trim().ToLowerInvariant()) {
        "vm" { "vm" }
        "standalone" { "standalone" }
        default { "invalid" }
    }
}

switch ($mode) {
    "vm" { Write-Output "  [state] marker: vm - VM rules apply; every session starts with -Down" }
    "standalone" { Write-Output "  [state] marker: standalone - VM rules intentionally off; write 'vm' to enable them" }
    "invalid" { Write-Output "  [state] marker: invalid content - treated as standalone; the user must rewrite it to 'vm' or 'standalone'" }
    default { Write-Output "  [state] marker: absent - unconfigured; the user must choose 'vm' or 'standalone'" }
}

Write-Output ""
if ($failures -gt 0) {
    Write-Output ("VM setup: FAIL - {0} check(s) need attention. Do not write 'vm'." -f $failures)
    exit 1
}
Write-Output "VM setup: PASS - write 'vm' to .sprout-workspace last, then run 'tools\sync.ps1 -Down'."
exit 0
