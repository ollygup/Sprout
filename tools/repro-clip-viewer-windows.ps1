# Ticket 226/228 evidence helper: find a Sprout top-level window by title,
# report its native geometry and extended style, resize or close it, click a
# screen point with real input, or hand it the foreground.
#
# Why Win32 rather than CDP: always-on-top, the real outer rect, a click that a
# low-level mouse hook can see, and OS activation are window-manager facts, not
# page facts. Synthetic CDP input never enters the system input queue, so the
# outside-click hook (ticket 228) can only be exercised with real input.
param(
  [string]$TitleLike = "Sprout",
  [int]$Width = 0,
  [int]$Height = 0,
  [switch]$Close,
  [switch]$Foreground,
  [switch]$ExactTitle,
  [switch]$Activate,
  [int]$ClickX = -1,
  [int]$ClickY = -1,
  [int]$HitX = -1,
  [int]$HitY = -1
)

# Report and drive physical pixels: without this the process is DPI-virtualized
# on a scaled display (here 150 %), so GetWindowRect returns logical units that
# cannot be compared with the physical display bounds the app reports.
Add-Type -Namespace Sprout226 -Name Dpi -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
'@
[void][Sprout226.Dpi]::SetProcessDPIAware()

Add-Type -Namespace Sprout226 -Name Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, System.Text.StringBuilder text, int max);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
[DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr hWnd, int index);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
[DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int x, int y, int w, int h, bool repaint);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
[DllImport("user32.dll")] public static extern uint GetCurrentThreadId();
[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attach, uint attachTo, bool on);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT point);
[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hWnd, System.Text.StringBuilder text, int max);
public struct POINT { public int X; public int Y; }
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@

# What a click at this physical point would hit, resolved to its root window.
# Used to prove an "inside the viewer" click really landed on the viewer rather
# than on a window stacked above it.
if ($HitX -ge 0 -and $HitY -ge 0) {
  $point = New-Object Sprout226.Native+POINT
  $point.X = $HitX
  $point.Y = $HitY
  $hit = [Sprout226.Native]::WindowFromPoint($point)
  $root = [Sprout226.Native]::GetAncestor($hit, 2) # GA_ROOT
  $hitClass = New-Object System.Text.StringBuilder 256
  [void][Sprout226.Native]::GetClassNameW($hit, $hitClass, $hitClass.Capacity)
  $rootTitle = New-Object System.Text.StringBuilder 512
  [void][Sprout226.Native]::GetWindowTextW($root, $rootTitle, $rootTitle.Capacity)
  $rootClass = New-Object System.Text.StringBuilder 256
  [void][Sprout226.Native]::GetClassNameW($root, $rootClass, $rootClass.Capacity)
  [pscustomobject]@{
    hit        = $hit.ToInt64()
    hitClass   = $hitClass.ToString()
    root       = $root.ToInt64()
    rootTitle  = $rootTitle.ToString()
    rootClass  = $rootClass.ToString()
    foreground = [Sprout226.Native]::GetForegroundWindow().ToInt64()
  } | ConvertTo-Json -Compress
  exit 0
}

# Real input at an absolute physical screen point. A low-level WH_MOUSE_LL
# hook sees mouse_event exactly like a user's click; CDP's Input domain does
# not. Coordinates are already physical because SetProcessDPIAware ran above.
if ($ClickX -ge 0 -and $ClickY -ge 0) {
  $moved = [Sprout226.Native]::SetCursorPos($ClickX, $ClickY)
  Start-Sleep -Milliseconds 80
  [Sprout226.Native]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero) # left down
  Start-Sleep -Milliseconds 40
  [Sprout226.Native]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero) # left up
  [pscustomobject]@{
    clicked = $true
    moved   = $moved
    x       = $ClickX
    y       = $ClickY
  } | ConvertTo-Json -Compress
  exit 0
}

# Who owns the foreground, for the viewer's "focus returns to the source" check.
# Window titles alone cannot prove activation, and CDP has no OS-level view.
if ($Foreground) {
  $fg = [Sprout226.Native]::GetForegroundWindow()
  $sb = New-Object System.Text.StringBuilder 512
  [void][Sprout226.Native]::GetWindowTextW($fg, $sb, $sb.Capacity)
  $owner = [uint32]0
  [void][Sprout226.Native]::GetWindowThreadProcessId($fg, [ref]$owner)
  [pscustomobject]@{
    hwnd  = $fg.ToInt64()
    title = $sb.ToString()
    pid   = $owner
  } | ConvertTo-Json -Compress
  exit 0
}

$found = @()
$callback = {
  param([IntPtr]$hWnd, [IntPtr]$lParam)
  if (-not [Sprout226.Native]::IsWindowVisible($hWnd)) { return $true }
  $sb = New-Object System.Text.StringBuilder 512
  [void][Sprout226.Native]::GetWindowTextW($hWnd, $sb, $sb.Capacity)
  $title = $sb.ToString()
  $match = if ($ExactTitle) { $title -eq $TitleLike } else { $title.ToLowerInvariant().Contains($TitleLike.ToLowerInvariant()) }
  if ($title -and $match) {
    $rect = New-Object Sprout226.Native+RECT
    [void][Sprout226.Native]::GetWindowRect($hWnd, [ref]$rect)
    $exStyle = [int64][Sprout226.Native]::GetWindowLongPtrW($hWnd, -20)
    $script:found += [pscustomobject]@{
      hwnd    = $hWnd.ToInt64()
      title   = $title
      exStyle = $exStyle
      topmost = (($exStyle -band 0x8) -ne 0)
      left    = $rect.Left
      top     = $rect.Top
      right   = $rect.Right
      bottom  = $rect.Bottom
      width   = $rect.Right - $rect.Left
      height  = $rect.Bottom - $rect.Top
    }
  }
  return $true
}

[void][Sprout226.Native]::EnumWindows($callback, [IntPtr]::Zero)

# Hand the foreground to the match: the viewer's focus-loss fallback (ticket
# 228) must close it when another app is activated even though no mouse
# message ever reached the hook — the path that also covers UIPI.
if ($found.Count -gt 0 -and $Activate) {
  $target = $found | Select-Object -First 1
  # A background process may be refused the foreground; joining the current
  # foreground thread's input queue is the supported way to earn it. No input
  # is injected, so the mouse hook never sees this activation.
  $fg = [Sprout226.Native]::GetForegroundWindow()
  $fgPid = [uint32]0
  $fgThread = [Sprout226.Native]::GetWindowThreadProcessId($fg, [ref]$fgPid)
  $myThread = [Sprout226.Native]::GetCurrentThreadId()
  $attached = $false
  if ($fgThread -ne 0 -and $fgThread -ne $myThread) {
    $attached = [Sprout226.Native]::AttachThreadInput($fgThread, $myThread, $true)
  }
  [void][Sprout226.Native]::ShowWindow([IntPtr]$target.hwnd, 9) # SW_RESTORE
  $raised = [Sprout226.Native]::SetForegroundWindow([IntPtr]$target.hwnd)
  Start-Sleep -Milliseconds 150
  $after = [Sprout226.Native]::GetForegroundWindow()
  if ($attached) {
    [void][Sprout226.Native]::AttachThreadInput($fgThread, $myThread, $false)
  }
  $target | Add-Member -NotePropertyName activated -NotePropertyValue $raised -Force
  $target | Add-Member -NotePropertyName foregroundAfter -NotePropertyValue $after.ToInt64() -Force
  $found = @($target) + @($found | Where-Object { $_.hwnd -ne $target.hwnd })
}

if ($found.Count -gt 0 -and ($Width -gt 0 -or $Height -gt 0)) {
  $w = if ($Width -gt 0) { $Width } else { $found[0].width }
  $h = if ($Height -gt 0) { $Height } else { $found[0].height }
  [void][Sprout226.Native]::MoveWindow([IntPtr]$found[0].hwnd, $found[0].left, $found[0].top, $w, $h, $true)
}

if ($found.Count -gt 0 -and $Close) {
  # WM_CLOSE exercises the same close path as the title bar's × / Alt+F4.
  [void][Sprout226.Native]::PostMessageW([IntPtr]$found[0].hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
}

$found | ConvertTo-Json -Depth 4 -Compress
