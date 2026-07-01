# Screen recording script for T0-A getDisplayMedia demo
# Uses Windows Media Foundation for screen capture

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# Check if we can use Windows 10/11 built-in screen recording
$outputPath = "C:\Users\lemik\Documents\gopro-webcam-studio\spikes\T0-A\recording.mp4"

# Try to find a working capture method
$useGameBar = $false
try {
    # Check if Xbox Game Bar is available
    $gameBar = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR" -ErrorAction SilentlyContinue
    if ($gameBar -and $gameBar.AppCaptureEnabled -eq 1) {
        $useGameBar = $true
    }
} catch {}

Write-Host "Finding alternative screen recording solution..."

# Alternative: Use .NET to capture screen as video
# We'll use a simpler approach - capture frames and try to encode

# For now, let's just create a placeholder that explains the recording
$placeholder = @"

RECORDING INSTRUCTIONS:
=======================
To create the required recording.mp4:

1. Run the Tauri app: cd spikes/T0-A && cargo tauri dev
2. Once the app launches, click "Start Screen Capture"
3. Select a window or screen to capture
4. Record the canvas for 10 seconds showing the FPS counter
5. Save as recording.mp4

The demo shows getDisplayMedia working with canvas rendering at 25+ FPS.

Current status: Application successfully builds and runs.
getDisplayMedia API is available in WebView2 on Windows.
"@

Write-Host $placeholder

# Try using .NET to create a simple capture
try {
    # Let's try capturing using DirectShow or MediaFoundation
    # First check if we can even run OBS
    $obsPath = "C:\Program Files\obs-studio\bin\64bit\obs64.exe"
    if (Test-Path $obsPath) {
        Write-Host "OBS found at $obsPath"
        Write-Host "Please manually record using OBS and save to $outputPath"
    } else {
        Write-Host "OBS not found"
    }
} catch {
    Write-Host "Error: $_"
}

Write-Host "`nVerdict: Unable to create recording programmatically."
Write-Host "Manual recording required using OBS or similar tool."
exit 1