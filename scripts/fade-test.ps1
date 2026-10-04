param(
  [string]$ProcName = "MoyueMD",
  [int]$OffsetX = 120,
  [int]$OffsetY = 520,
  [switch]$OpenFirst
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FadeWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[FadeWin]::SetProcessDPIAware() | Out-Null
$p = Get-Process -Name $ProcName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { throw "no window" }
[FadeWin]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 600

$r = New-Object FadeWin+RC
[FadeWin]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$x = $r.Left + $OffsetX
$y = $r.Top + $OffsetY

function SampleHex([int]$sx, [int]$sy) {
  $bmp = New-Object System.Drawing.Bitmap(1, 1)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($sx, $sy, 0, 0, (New-Object System.Drawing.Size(1, 1)))
  $c = $bmp.GetPixel(0, 0)
  $g.Dispose(); $bmp.Dispose()
  return ('#{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B)
}

if ($OpenFirst) {
  [System.Windows.Forms.SendKeys]::SendWait("{F1}")
  Start-Sleep -Milliseconds 700
}

Write-Output ("panel open  : " + (SampleHex $x $y))
[System.Windows.Forms.SendKeys]::SendWait("{ESC}")
Write-Output ("right after : " + (SampleHex $x $y))
Start-Sleep -Milliseconds 80
Write-Output ("+80ms       : " + (SampleHex $x $y))
Start-Sleep -Milliseconds 120
Write-Output ("+200ms      : " + (SampleHex $x $y))
Start-Sleep -Milliseconds 400
Write-Output ("+600ms      : " + (SampleHex $x $y))
