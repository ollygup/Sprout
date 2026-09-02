param(
    [string]$AppDataRoot = "$env:TEMP\sprout-ticket-123-repro-profile",
    [string]$WebViewRoot = "$env:TEMP\sprout-ticket-123-repro-webview"
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class SproutLifecycleProbe {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumProc callback, IntPtr lparam);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern bool IsHungAppWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern int GetWindowLong(IntPtr hwnd, int index);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);

    [DllImport("user32.dll")]
    public static extern bool ShowWindowAsync(IntPtr hwnd, int command);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hwnd,
        uint message,
        IntPtr wparam,
        IntPtr lparam,
        uint flags,
        uint timeout,
        out IntPtr result
    );

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr hwnd, System.Text.StringBuilder text, int count);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassName(IntPtr hwnd, System.Text.StringBuilder text, int count);
}
'@

function Get-SproutWindows([int]$ProcessId) {
    $windows = [Collections.Generic.List[object]]::new()
    [SproutLifecycleProbe]::EnumWindows({
        param($hwnd, $lparam)
        $ownerProcessId = 0
        [SproutLifecycleProbe]::GetWindowThreadProcessId($hwnd, [ref]$ownerProcessId) | Out-Null
        if ($ownerProcessId -eq $ProcessId) {
            $title = [Text.StringBuilder]::new(256)
            $class = [Text.StringBuilder]::new(256)
            $rect = [SproutLifecycleProbe+Rect]::new()
            $hung = [SproutLifecycleProbe]::IsHungAppWindow($hwnd)
            if (-not $hung) {
                [SproutLifecycleProbe]::GetWindowText($hwnd, $title, 256) | Out-Null
            }
            [SproutLifecycleProbe]::GetClassName($hwnd, $class, 256) | Out-Null
            [SproutLifecycleProbe]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
            $extendedStyle = [SproutLifecycleProbe]::GetWindowLong($hwnd, -20)
            $windows.Add([pscustomobject]@{
                hwnd       = $hwnd.ToInt64()
                title      = $title.ToString()
                class      = $class.ToString()
                width      = $rect.Right - $rect.Left
                height     = $rect.Bottom - $rect.Top
                visible    = [SproutLifecycleProbe]::IsWindowVisible($hwnd)
                hung       = $hung
                layered    = ($extendedStyle -band 0x80000) -ne 0
                toolWindow = ($extendedStyle -band 0x80) -ne 0
            })
        }
        return $true
    }, [IntPtr]::Zero) | Out-Null
    return @($windows)
}

if (Get-Process sprout -ErrorAction SilentlyContinue) {
    throw "Close every existing Sprout process before running this repro."
}
try {
    Invoke-WebRequest -UseBasicParsing -Uri "http://localhost:1420/" -TimeoutSec 2 | Out-Null
}
catch {
    throw "Start Vite on port 1420 first: npm.cmd run dev"
}
if (-not (Test-Path -LiteralPath (Join-Path $AppDataRoot "Sprout\sprout.db"))) {
    throw "The isolated profile is missing Sprout\sprout.db: $AppDataRoot"
}

$env:LOCALAPPDATA = [IO.Path]::GetFullPath($AppDataRoot)
$env:WEBVIEW2_USER_DATA_FOLDER = [IO.Path]::GetFullPath($WebViewRoot)
$executable = "C:\Sprout\src-tauri\target\debug\sprout.exe"
$launched = [Collections.Generic.List[int]]::new()
$exitCode = 2

try {
    Write-Output "[DEBUG-LIFECYCLE] launching first process"
    $first = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
    $launched.Add($first.Id)
    Start-Sleep -Seconds 2
    Write-Output "[DEBUG-LIFECYCLE] capturing boot windows"
    $before = Get-SproutWindows $first.Id

    Write-Output "[DEBUG-LIFECYCLE] sending initial open signal"
    $second = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
    $launched.Add($second.Id)
    Start-Sleep -Seconds 3
    Write-Output "[DEBUG-LIFECYCLE] capturing initially opened main"
    $opened = Get-SproutWindows $first.Id
    $main = @($opened | Where-Object { $_.class -eq "Tauri Window" -and $_.width -gt 700 })
    if ($main.Count -ne 1) {
        throw "Expected one main window after the initial open signal."
    }

    Write-Output "[DEBUG-LIFECYCLE] minimizing then foregrounding main"
    [SproutLifecycleProbe]::ShowWindowAsync([IntPtr]$main[0].hwnd, 6) | Out-Null
    Start-Sleep -Milliseconds 200
    $third = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
    $launched.Add($third.Id)
    Start-Sleep -Seconds 1

    Write-Output "[DEBUG-LIFECYCLE] closing main"
    $closeResult = [IntPtr]::Zero
    [SproutLifecycleProbe]::SendMessageTimeout(
        [IntPtr]$main[0].hwnd,
        0x0010,
        [IntPtr]::Zero,
        [IntPtr]::Zero,
        2,
        2000,
        [ref]$closeResult
    ) | Out-Null
    Start-Sleep -Milliseconds 100

    Write-Output "[DEBUG-LIFECYCLE] reopening after close"
    $fourth = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
    $launched.Add($fourth.Id)
    Start-Sleep -Seconds 3
    Write-Output "[DEBUG-LIFECYCLE] capturing reopened main"
    $reopened = Get-SproutWindows $first.Id
    $reopenedMain = @($reopened | Where-Object { $_.class -eq "Tauri Window" -and $_.width -gt 700 })
    if ($reopenedMain.Count -eq 1) {
        [SproutLifecycleProbe]::ShowWindowAsync([IntPtr]$reopenedMain[0].hwnd, 6) | Out-Null
    }
    Start-Sleep -Milliseconds 200

    Write-Output "[DEBUG-LIFECYCLE] sending final open signal"
    $fifth = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
    $launched.Add($fifth.Id)
    Start-Sleep -Seconds 3
    Write-Output "[DEBUG-LIFECYCLE] capturing final state"
    $after = Get-SproutWindows $first.Id

    [pscustomobject]@{
        launchedPids  = @($launched)
        windowsBefore = $before
        windowsOpened = $opened
        windowsReopened = $reopened
        windowsAfter  = $after
    } | ConvertTo-Json -Depth 5

    $main = @($after | Where-Object { $_.class -eq "Tauri Window" -and $_.width -gt 700 })
    $hung = @($after | Where-Object { $_.hung }).Count -gt 0
    if ($main.Count -eq 0 -or $hung) {
        Write-Error "RED: second-instance open left the main lifecycle missing or hung."
        $exitCode = 1
    } else {
        Write-Output "GREEN: main window opened and the Sprout event loop remained responsive."
        $exitCode = 0
    }
}
finally {
    foreach ($processId in $launched) {
        if (Get-Process -Id $processId -ErrorAction SilentlyContinue) {
            Stop-Process -Id $processId -Force
        }
    }
}

exit $exitCode
