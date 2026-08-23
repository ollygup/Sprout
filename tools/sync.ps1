<#
.SYNOPSIS
  Two-way sync between the working copy (C:\Sprout) and the git master on the
  share (\\vmware-host\Shared Folders\Projects\Sprout), guarded so the share's
  git state is never silently clobbered.

.DESCRIPTION
  The share is the git source of truth (AGENTS.md); C:\Sprout is where builds
  run (UNC paths break npm/cargo). A plain robocopy up-overwrites whatever the
  other device committed, producing dirty trees and merge conflicts. This
  script records a hash snapshot of the share at session start (-Down) and at
  session end (-Up) only copies a file up when the share's copy is unchanged
  since that snapshot - anything the other device touched mid-session is
  skipped and reported for explicit resolution.

  -Down (run at session start)
      Refreshes C:\Sprout from the share (add/update only, never deletes),
      then snapshots every share file's content hash. Run this before working.
  -Up   (run after finishing, default)
      Copies C:\Sprout changes to the share, but only for files whose share
      copy still matches the snapshot. New files are copied; files deleted on
      the share are left alone (never resurrected) and reported. Skips the
      build dirs and .sync-state.json itself.

  Hashes are line-ending-insensitive (CRLF == LF), so git checkouts that
  normalize EOLs never block a sync. If no snapshot exists, -Up refuses to
  run - start with -Down.

  After -Up, verify with:  tools\sync.ps1 -Up   (expect "0 to copy").
#>
param(
    [switch]$Down,
    [switch]$Up
)

$ErrorActionPreference = "Stop"

$share = "\\vmware-host\Shared Folders\Projects\Sprout"
$local = "C:\Sprout"
$stateFile = Join-Path $local ".sync-state.json"
# Same list as the AGENTS.md robocopy /XD (minus .git) - checked against
# EVERY path segment, so nested build dirs like src-tauri\target are caught.
$excludeSegments = @("node_modules", "target", "build", ".svelte-kit", ".vscode", ".codegraph", ".git")
$excludeFiles = @(".sync-state.json")
# Sanity ceiling: a legit session touches a handful of files; anything near
# this is a runaway copy (e.g. build output) - abort before writing.
$maxCopy = 500

function Is-Excluded([string]$rel) {
    if ($rel -in $excludeFiles) { return $true }
    foreach ($segment in $rel -split "\\") {
        if ($segment -in $excludeSegments) { return $true }
    }
    $false
}

function Get-Rel([string]$root, [string]$full) {
    $full.Substring($root.Length + 1)
}

function Get-NormHash([string]$path) {
    $text = [System.IO.File]::ReadAllText($path)
    $text = $text.Replace("`r`n", "`n")
    $sha = [System.Security.Cryptography.SHA256]::Create()
    [BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($text))).Replace("-", "")
}

function Get-Tree([string]$root) {
    Get-ChildItem -LiteralPath $root -Recurse -File -Force -ErrorAction SilentlyContinue |
        Where-Object { -not (Is-Excluded (Get-Rel $root $_.FullName)) }
}

function Assert-CopyBudget {
    param([int]$count)
    if ($count -gt $maxCopy) {
        Write-Error "Aborting: $count files would be copied (ceiling is $maxCopy). This looks like a runaway sync - investigate before retrying."
        exit 1
    }
}

function Save-State([hashtable]$hashes) {
    $payload = @{
        taken = (Get-Date).ToString("o")
        files = $hashes
    }
    $payload | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $stateFile -Encoding UTF8
}

function Load-State {
    if (-not (Test-Path -LiteralPath $stateFile)) {
        return $null
    }
    $json = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
    $hashes = @{}
    foreach ($prop in $json.files.PSObject.Properties) {
        $hashes[$prop.Name] = $prop.Value
    }
    $hashes
}

if ($Down) {
    Write-Output "== Sync down (share -> C:\Sprout) =="
    $plan = @()
    foreach ($file in Get-Tree $share) {
        $rel = Get-Rel $share $file.FullName
        $localFile = Join-Path $local $rel
        if (-not (Test-Path -LiteralPath $localFile) -or
            (Get-NormHash $file.FullName) -ne (Get-NormHash $localFile)) {
            $plan += , @($rel, $file.FullName, $localFile)
        }
    }
    Assert-CopyBudget $plan.Count
    $copied = 0
    foreach ($item in $plan) {
        $rel = $item[0]
        $localFile = $item[2]
        New-Item -ItemType Directory -Path (Split-Path -Parent $localFile) -Force | Out-Null
        Copy-Item -LiteralPath $item[1] -Destination $localFile -Force
        Write-Output "  updated:  $rel"
        $copied++
    }
    Write-Output "Down: $copied copied."
    $hashes = @{}
    foreach ($file in Get-Tree $share) {
        $rel = Get-Rel $share $file.FullName
        $hashes[$rel] = (Get-NormHash $file.FullName)
    }
    Save-State $hashes
    Write-Output "Snapshot taken ($($hashes.Count) files)."
    exit 0
}

if ($Up -or -not $Down) {
    $hashes = Load-State
    if ($null -eq $hashes) {
        Write-Error "No snapshot found. Run 'tools\sync.ps1 -Down' first - the guard refuses to sync blind."
        exit 1
    }
    Write-Output "== Sync up (C:\Sprout -> share) =="
    $copied = 0
    $skipped = @()
    $newCount = 0
    $deleted = @()
    $plan = @()
    foreach ($file in Get-Tree $local) {
        $rel = Get-Rel $local $file.FullName
        $shareFile = Join-Path $share $rel
        $localHash = Get-NormHash $file.FullName
        if (-not (Test-Path -LiteralPath $shareFile)) {
            if ($hashes.ContainsKey($rel)) {
                $deleted += $rel
                continue
            }
            $plan += , @($rel, $file.FullName, $shareFile, "new")
            continue
        }
        $shareHash = Get-NormHash $shareFile
        if (-not $hashes.ContainsKey($rel)) {
            $skipped += $rel
            continue
        }
        if ($shareHash -ne $hashes[$rel]) {
            $skipped += $rel
            continue
        }
        if ($localHash -ne $shareHash) {
            $plan += , @($rel, $file.FullName, $shareFile, "updated")
        }
    }
    Assert-CopyBudget $plan.Count
    foreach ($item in $plan) {
        $rel = $item[0]
        $shareFile = $item[2]
        New-Item -ItemType Directory -Path (Split-Path -Parent $shareFile) -Force | Out-Null
        Copy-Item -LiteralPath $item[1] -Destination $shareFile -Force
        $hashes[$rel] = (Get-NormHash $item[1])
        if ($item[3] -eq "new") { $newCount++ }
        Write-Output "  $($item[3]): $rel"
        $copied++
    }
    Save-State $hashes
    Write-Output "Up: $copied copied, $($newCount) of them new; $($skipped.Count) kept on share because the other device changed them:"
    foreach ($rel in $skipped) { Write-Output "  SHARE-NEWER (skipped): $rel" }
    foreach ($rel in $deleted) { Write-Output "  SHARE-DELETED (left alone): $rel" }
    exit 0
}