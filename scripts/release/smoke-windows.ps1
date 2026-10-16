# Clean-machine smoke test checklist (manual; prints the steps and verifies the installer exists)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")
$setup = Get-ChildItem -Recurse "target\release\bundle\nsis" -Include *-setup.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $setup) { throw "No NSIS installer found. Run scripts\release\build-windows.ps1 first." }
Write-Host "Installer: $($setup.FullName)"
@"
Smoke test on a machine WITHOUT Rust/Node/Python:
 1. Run the installer as a normal user (no admin prompt expected; currentUser mode).
 2. Launch EthnoFlow Studio. Input/output selects list WASAPI device names.
 3. Record 10 s from the microphone: meter moves, no CLIP at normal level.
 4. Stop: waveform appears; sample rate and channels match the device.
 5. Play with headphones: audio is heard, playhead moves at real time, ends at the end.
 6. Export 24-bit WAV; open it in a DAW (Reaper/Audacity): length and content match.
 7. Import the same file: identical peak/RMS/duration in the info row.
 8. Switch language to EN and back.
 9. Uninstall cleanly from Settings > Apps.
Record results in docs/CHANGELOG.md under 'Phase 0 Windows verification'.
"@
