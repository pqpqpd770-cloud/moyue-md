# ASCII only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
# One browser step per call: navigate / click / paste / send keys, then capture.
# Text is entered by pasting, so Caps Lock and IME state cannot corrupt it.
param(
  [string]$ProcName = "chrome",
  [string]$Out = "step.png",
  [string]$Nav = "",
  [int]$X = -1,
  [int]$Y = -1,
  [string]$Paste = "",
  [string]$Keys = "",
  [switch]$Enter,
  [int]$WaitMs = 1200
)

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class StepWin {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[StepWin]::SetProcessDPIAware() | Out-Null
$proc = Get-Process -Name $ProcName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "no window for $ProcName" }
[StepWin]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500

$rect = New-Object StepWin+RC
[StepWin]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null

function Send([string]$keys) {
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep -Milliseconds 250
}

if ($Nav -ne "") {
  Set-Clipboard -Value $Nav
  Send "^l"
  Send "^v"
  Send "{ENTER}"
  Start-Sleep -Milliseconds $WaitMs
}

if ($X -ge 0 -and $Y -ge 0) {
  [StepWin]::SetCursorPos(($rect.Left + $X), ($rect.Top + $Y)) | Out-Null
  Start-Sleep -Milliseconds 180
  [StepWin]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  [StepWin]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 300
}

if ($Paste -ne "") {
  Set-Clipboard -Value $Paste
  Start-Sleep -Milliseconds 150
  Send "^v"
}

if ($Keys -ne "") { Send $Keys }
if ($Enter) { Send "{ENTER}" }

Start-Sleep -Milliseconds $WaitMs
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
$bmp = New-Object System.Drawing.Bitmap($width, $height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "step done -> $Out"
