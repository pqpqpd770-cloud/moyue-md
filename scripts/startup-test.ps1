# ASCII only (Windows PowerShell 5.1 reads BOM-less files as ANSI).
# Polls for the window, then shoots three frames as fast as it can and reports
# where the red seal sits in each - a running stamp animation shows up as a
# changing bounding box.
param(
  [string]$Exe = ".\target\debug\MoyueMD.exe",
  [string]$Doc = "samples\demo.md",
  [string]$OutDir = "."
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class BootWin {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  public struct RC { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

[BootWin]::SetProcessDPIAware() | Out-Null
Get-Process MoyueMD -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 600

Start-Process -FilePath $Exe -ArgumentList "`"$Doc`"" | Out-Null

$proc = $null
for ($i = 0; $i -lt 150 -and -not $proc; $i++) {
  Start-Sleep -Milliseconds 50
  $cand = Get-Process MoyueMD -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($cand) { $proc = $cand }
}
if (-not $proc) { throw "window never appeared" }

function CurrentRect() {
  $r = New-Object BootWin+RC
  [BootWin]::GetWindowRect($proc.MainWindowHandle, [ref]$r) | Out-Null
  return $r
}

# Wait until the window has its real size, otherwise we shoot a stale rectangle.
for ($i = 0; $i -lt 100; $i++) {
  $probe = CurrentRect
  if (($probe.Right - $probe.Left) -gt 700 -and ($probe.Bottom - $probe.Top) -gt 400) { break }
  Start-Sleep -Milliseconds 20
}

function Shoot([string]$name) {
  $rect = CurrentRect
  $width = $rect.Right - $rect.Left
  $height = $rect.Bottom - $rect.Top
  Write-Output ("  [{0}] rect = {1},{2} {3}x{4}" -f $name, $rect.Left, $rect.Top, $width, $height)
  $bmp = New-Object System.Drawing.Bitmap($width, $height)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($width, $height)))
  $g.Dispose()
  $path = Join-Path $OutDir $name
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  return $path
}

# The seal is the only saturated red on screen (#c4493a).
function SealBox([string]$path) {
  $bmp = New-Object System.Drawing.Bitmap($path)
  $minX = 99999; $minY = 99999; $maxX = -1; $maxY = -1; $count = 0
  for ($y = 0; $y -lt [Math]::Min(70, $bmp.Height); $y++) {
    for ($x = 0; $x -lt [Math]::Min(120, $bmp.Width); $x++) {
      $c = $bmp.GetPixel($x, $y)
      if ($c.R -gt 150 -and $c.G -lt 110 -and $c.B -lt 100) {
        $count++
        if ($x -lt $minX) { $minX = $x }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }
  $bmp.Dispose()
  if ($count -eq 0) { return "no red pixels" }
  return "$count px, box=$($maxX-$minX+1)x$($maxY-$minY+1) at ($minX,$minY)"
}

$a = Shoot "boot-a.png"
Start-Sleep -Milliseconds 260
$b = Shoot "boot-b.png"
Start-Sleep -Milliseconds 260
$c = Shoot "boot-c.png"
Start-Sleep -Seconds 3
$d = Shoot "boot-d.png"

Write-Output "frame A (t+0ms)    : $(SealBox $a)"
Write-Output "frame B (t+~300ms) : $(SealBox $b)"
Write-Output "frame C (t+~600ms) : $(SealBox $c)"
Write-Output "frame D (t+3.6s)   : $(SealBox $d)"

Get-Process MoyueMD -ErrorAction SilentlyContinue | Stop-Process -Force
