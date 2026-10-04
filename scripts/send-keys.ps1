# ASCII only. Sends keystrokes to whatever window currently has focus, without
# stealing focus (needed for native dialogs opened by the browser).
param(
  [string]$Keys = "",
  [string]$Paste = "",
  [switch]$Clear,
  [switch]$Enter,
  [int]$WaitMs = 600
)

Add-Type -AssemblyName System.Windows.Forms

if ($Paste -ne "") { Set-Clipboard -Value $Paste; Start-Sleep -Milliseconds 150 }
if ($Clear) {
  [System.Windows.Forms.SendKeys]::SendWait("{END}")
  Start-Sleep -Milliseconds 120
  [System.Windows.Forms.SendKeys]::SendWait("{BS}" * 90)
  Start-Sleep -Milliseconds 200
}
if ($Paste -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait("^v")
  Start-Sleep -Milliseconds 300
}
if ($Keys -ne "") {
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds 200
}
if ($Enter) {
  [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
}
Start-Sleep -Milliseconds $WaitMs
Write-Output "keys sent"
