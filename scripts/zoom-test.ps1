# ASCII only: Windows PowerShell 5.1 reads BOM-less files as ANSI, and non-ASCII
# comments can then swallow the newline and break the parser.
param(
  [string]$ProcName = "MoyueMD",
  [int]$Steps = 4,
  [int]$Row = 420,
  [string]$OutDir = "."
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ZoomWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[ZoomWin]::SetProcessDPIAware() | Out-Null
$proc = Get-Process -Name $ProcName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "no window for $ProcName" }
[ZoomWin]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 700

$rect = New-Object ZoomWin+RC
[ZoomWin]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

# WebView2 only takes keyboard focus after a click inside the page, so click the
# empty desk margin (no interactive element there) before sending keys.
[ZoomWin]::SetCursorPos(($rect.Left + 300), ($rect.Top + [int]($height * 0.7))) | Out-Null
Start-Sleep -Milliseconds 200
[ZoomWin]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
[ZoomWin]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 500

function Capture([string]$name) {
  $bmp = New-Object System.Drawing.Bitmap($width, $height)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($width, $height)))
  $g.Dispose()
  $path = Join-Path $OutDir $name
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  return $path
}

# x of the page's left rule (--rule #333A3B), scanning right of the TOC rule
function PageLeft([string]$path, [int]$row) {
  $bmp = New-Object System.Drawing.Bitmap($path)
  $found = -1
  for ($x = 280; $x -lt ($bmp.Width - 40); $x++) {
    $c = $bmp.GetPixel($x, $row)
    if ([Math]::Abs($c.R - 0x33) -le 6 -and [Math]::Abs($c.G - 0x3A) -le 6 -and [Math]::Abs($c.B - 0x3B) -le 6) {
      $found = $x
      break
    }
  }
  $bmp.Dispose()
  return $found
}

$before = Capture "zoom-before.png"
$beforeX = PageLeft $before $Row

for ($i = 0; $i -lt $Steps; $i++) {
  [System.Windows.Forms.SendKeys]::SendWait("^=")   # Ctrl+=
  Start-Sleep -Milliseconds 55
}

$during = Capture "zoom-during.png"
Start-Sleep -Milliseconds 500
$after = Capture "zoom-after.png"

$duringX = PageLeft $during $Row
$afterX = PageLeft $after $Row

Write-Output "page left edge: before=$beforeX during=$duringX after=$afterX"
if ($beforeX -gt 0 -and $afterX -gt 0 -and $duringX -ne $afterX) {
  Write-Output "RESULT: width was still moving when captured -> transition is running"
} else {
  Write-Output "RESULT: no intermediate value seen (animation finished early, or missing)"
}
