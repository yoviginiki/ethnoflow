param([string]$Out)
# Screenshot of the primary screen for docs/audits/screenshots (fidelity contract: real
# screenshots from the running app). Usage, from the repo root:
#   powershell -ExecutionPolicy Bypass -File scripts/dev/shot.ps1 -Out docs/audits/screenshots/<date>-<screen>.png
# Navigate the app first through the assistant bridge:  node scripts/assistant/ef.mjs send '{"op":"screen","screen":"references"}'
#
# DPI-aware capture: without SetProcessDPIAware the process sees a scaled (1536x864) view
# of a 1920x1080 screen at 125 % and the right/bottom 20 % of the screen are missing.
Add-Type @"
using System; using System.Runtime.InteropServices;
public class Dpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
"@
[Dpi]::SetProcessDPIAware() | Out-Null
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
