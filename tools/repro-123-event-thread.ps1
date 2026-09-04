# Ticket 123 regression guard: asserts the dock-mark -> main-window open path
# does NOT block the Tauri event thread.
#
# Bug pattern (user symptom: dock-mark click opens an invisible main frame and
# the main window then ignores X): `open_main_window` sleeps synchronously
# (800 ms close-grace + up to 7x120 ms zombie retries). Every post-start entry
# point -- dock mark IPC (`open_main_window_cmd` / `open_sprout_cmd` ->
# `tray::open_sprout`), tray menu, single-instance hook -- runs ON the event
# thread, which is also the thread that must process the queued `destroy()` for
# the "main" label to clear. Sleep blocks it, so the retry loop waits for work
# it prevents (self-deadlock): the rebuilt window never reveals (invisible
# frame) and CloseRequested never processes (X dead).
#
# RED (exit 1): blocking `open_main_window` reachable synchronously from an
#   event-thread entry point with no off-thread seam.
# GREEN (exit 0): all post-start entry points route through an off-thread
#   single-flight seam (`spawn_blocking`); boot path stays sync.
$ErrorActionPreference = "Stop"

$lib  = Get-Content -Raw -LiteralPath "C:\Sprout\src-tauri\src\lib.rs"
$tray = Get-Content -Raw -LiteralPath "C:\Sprout\src-tauri\src\tray.rs"

$failures = @()

# 1. The sleeping function must exist (it owns the retry grace).
if ($lib -notmatch "pub\(crate\) fn open_main_window") {
    $failures += "open_main_window seam missing"
}
$blockingSleeps = ([regex]::Matches($lib, "std::thread::sleep")).Count
if ($blockingSleeps -eq 0) {
    $failures += "expected blocking sleeps in open_main_window (seam changed?)"
}

# 2. An off-thread single-flight seam must exist.
if ($lib -notmatch "request_open_main_window") {
    $failures += "no request_open_main_window off-thread seam"
}
if ($lib -notmatch "main_window_opening") {
    $failures += "no main_window_opening single-flight flag"
}
if ($lib -notmatch "spawn_blocking\(move \|\|") {
    $failures += "open_main_window not dispatched via spawn_blocking"
}

# 3. Post-start entry points must use the seam, never the blocking call.
# 3a. Dock-mark / generic IPC command.
$cmdBlock = [regex]::Match($lib, "fn open_main_window_cmd.*?^}", "Singleline,Multiline").Value
if ($cmdBlock -match "crate::open_main_window\(") {
    $failures += "open_main_window_cmd calls blocking open_main_window synchronously (dock-mark path hangs event thread)"
}
# 3b. Tray menu path (also serves open_sprout_cmd, i.e. the current dock-mark handler).
if ($tray -match "crate::open_main_window\(") {
    $failures += "tray::open_sprout calls blocking open_main_window synchronously (tray/dock IPC path hangs event thread)"
}
# 3c. Single-instance hook (second launch while main closed).
$singleBlock = [regex]::Match($lib, "single_instance::init.*?\}\)\)", "Singleline").Value
if ($singleBlock -match "open_main_window\(" -and $singleBlock -notmatch "request_open_main_window") {
    $failures += "single-instance hook calls blocking open_main_window synchronously"
}

# 4. Boot path must stay synchronous (no close-race there, no added delay).
$setupBlock = [regex]::Match($lib, "if !autostart_boot.*?open_main_window", "Singleline").Value
if ($setupBlock -notmatch "open_main_window") {
    $failures += "boot path no longer uses open_main_window directly (unexpected)"
}

if ($failures.Count -gt 0) {
    Write-Output "RED: event-thread block reachable on the dock-mark -> main-window path"
    $failures | ForEach-Object { Write-Output "  - $_" }
    exit 1
}
Write-Output "GREEN: dock-mark -> main-window path is off-thread single-flight; boot stays sync."
exit 0
