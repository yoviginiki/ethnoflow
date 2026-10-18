param([string]$Out, [string]$Key = "")
# Screenshot of the primary screen for docs/audits/screenshots (fidelity contract: real
# screenshots from the running app). Usage, from the repo root:
#   powershell -ExecutionPolicy Bypass -File scripts/dev/shot.ps1 -Out docs/audits/screenshots/<date>-<screen>.png
# Navigate the app first through the assistant bridge:  node scripts/assistant/ef.mjs send '{"op":"screen","screen":"references"}'
#
# The app window is brought to the front first and the capture is REFUSED unless the
# foreground window really belongs to ethnoflow-desktop (Windows may deny foreground
# changes; capturing whatever else is on screen is never acceptable). DPI-aware capture:
# without SetProcessDPIAware the process sees a scaled view and the right/bottom are cut.
Add-Type @"
using System; using System.Runtime.InteropServices;
public class Win { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid); }
"@
[Win]::SetProcessDPIAware() | Out-Null
$app = Get-Process -Name ethnoflow-desktop -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $app) { Write-Error "ethnoflow-desktop is not running: no capture"; exit 2 }
[Win]::ShowWindow($app.MainWindowHandle, 3) | Out-Null   # SW_MAXIMIZE: the contract wants full-size screenshots
[Win]::SetForegroundWindow($app.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 800
$fg = [Win]::GetForegroundWindow()
$pid2 = 0
[Win]::GetWindowThreadProcessId($fg, [ref]$pid2) | Out-Null
if ($pid2 -ne $app.Id) { Write-Error "the app is not the foreground window (pid $pid2): no capture"; exit 3 }
if ($Key -ne "") {
  # e.g. -Key "{END}" scrolls the page to the bottom before the capture
  Add-Type -AssemblyName System.Windows.Forms
  [System.Windows.Forms.SendKeys]::SendWait($Key)
  Start-Sleep -Milliseconds 600
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$dir = Split-Path $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
"$($b.Width)x$($b.Height) -> $Out"
