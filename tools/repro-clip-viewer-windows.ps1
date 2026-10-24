# Ticket 226 evidence helper: find a Sprout top-level window by title, report its
# native geometry and extended style, and optionally resize or close it.
#
# Why Win32 rather than CDP: always-on-top and the real outer rect are window
# manager facts, not page facts. This script is the one place the viewer's
# native state is observed and driven, so the repro stays honest.
param(
  [string]$TitleLike = "Sprout",
  [int]$Width = 0,
  [int]$Height = 0,
  [switch]$Close,
  [switch]$Foreground
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
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@

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
  if ($title -and $title.ToLowerInvariant().Contains($TitleLike.ToLowerInvariant())) {
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
