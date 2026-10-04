# ASCII only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
# Focus the app window, click at an offset, then capture the window.
param(
  [string]$ProcName = "MoyueMD",
  [int]$X = 1152,
  [int]$Y = 25,
  [string]$Out = "click.png",
  [int]$WaitMs = 800,
  [string]$Keys = "",
  [string]$Type = "",
  [switch]$NoClick
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ClickWin {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[ClickWin]::SetProcessDPIAware() | Out-Null
$proc = Get-Process -Name $ProcName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "no window" }
[ClickWin]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 600

$rect = New-Object ClickWin+RC
[ClickWin]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null

if (-not $NoClick) {
  [ClickWin]::SetCursorPos(($rect.Left + $X), ($rect.Top + $Y)) | Out-Null
  Start-Sleep -Milliseconds 200
  [ClickWin]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  [ClickWin]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 200
}
if ($Keys -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds 200
}
if ($Type -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Type)
  Start-Sleep -Milliseconds 200
}
Start-Sleep -Milliseconds $WaitMs

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
$bmp = New-Object System.Drawing.Bitmap($width, $height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "captured $Out"
