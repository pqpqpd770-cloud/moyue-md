# ASCII only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
# Ink-and-print layout has no desk margin, so the page fills the view and there
# is no left rule to track. Instead: capture mid-flight and settled, and compare
# a band of body text. A difference means the size was still animating.
param(
  [string]$ProcName = "MoyueMD",
  [int]$Steps = 1,
  [string]$OutDir = "."
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ZoomDiffWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[ZoomDiffWin]::SetProcessDPIAware() | Out-Null
$proc = Get-Process -Name $ProcName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "no window for $ProcName" }
[ZoomDiffWin]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 700

$rect = New-Object ZoomDiffWin+RC
[ZoomDiffWin]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

# WebView2 needs a click inside the page before it will take keyboard focus.
[ZoomDiffWin]::SetCursorPos(($rect.Left + 400), ($rect.Top + [int]($height * 0.7))) | Out-Null
Start-Sleep -Milliseconds 200
[ZoomDiffWin]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
[ZoomDiffWin]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
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

# Body text band, avoiding the titlebar, TOC and scrollbars.
function Band($path) {
  $bmp = New-Object System.Drawing.Bitmap($path)
  $sum = 0
  for ($y = 120; $y -lt 420; $y += 2) {
    for ($x = 460; $x -lt 1180; $x += 2) {
      $c = $bmp.GetPixel($x, $y)
      $sum += ($c.R * 3 + $c.G * 5 + $c.B * 7)
    }
  }
  $bmp.Dispose()
  return $sum
}

function BandDiff($a, $b) {
  $ba = New-Object System.Drawing.Bitmap($a)
  $bb = New-Object System.Drawing.Bitmap($b)
  $diff = 0
  for ($y = 120; $y -lt 420; $y += 2) {
    for ($x = 460; $x -lt 1180; $x += 2) {
      $ca = $ba.GetPixel($x, $y); $cb = $bb.GetPixel($x, $y)
      if ([Math]::Abs($ca.R - $cb.R) -gt 24 -or [Math]::Abs($ca.G - $cb.G) -gt 24 -or [Math]::Abs($ca.B - $cb.B) -gt 24) { $diff++ }
    }
  }
  $ba.Dispose(); $bb.Dispose()
  return $diff
}

$before = Capture "zoom-before.png"

for ($i = 0; $i -lt $Steps; $i++) {
  [System.Windows.Forms.SendKeys]::SendWait("^=")
}

# Let the first repaint land, then capture: an instant change is settled by now,
# a 100-180ms transition is still mid-flight.
Start-Sleep -Milliseconds 60
$during = Capture "zoom-during.png"
Start-Sleep -Milliseconds 500
$after = Capture "zoom-after.png"

$changed = BandDiff $before $after
$stillMoving = BandDiff $during $after
Write-Output "pixels changed by the zoom: $changed ; of those still moving at capture time: $stillMoving"
if ($changed -lt 50) {
  Write-Output "RESULT: font size did not change (keypress lost?)"
} elseif ($stillMoving -gt 0) {
  Write-Output "RESULT: transition is running (mid-flight frame differs from the settled one)"
} else {
  Write-Output "RESULT: change was instant (no intermediate frame)"
}
