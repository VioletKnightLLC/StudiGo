Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap(32, 32)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::FromArgb(0, 120, 212))
$g.Dispose()
$icon = [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
$fs = [System.IO.FileStream]::new("icon.ico", [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$bmp.Dispose()
Write-Host "Created icon.ico"
