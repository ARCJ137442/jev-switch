param(
    [Parameter(Mandatory = $true)]
    [string]$ExecutablePath,
    [string]$IconPath = (Join-Path $PSScriptRoot '../src-tauri/icons/icon.ico')
)

$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path -LiteralPath $ExecutablePath).Path
$iconSourcePath = (Resolve-Path -LiteralPath $IconPath).Path

$expectedIcon = [System.Drawing.Icon]::new($iconSourcePath)
$embeddedIcon = [System.Drawing.Icon]::ExtractAssociatedIcon($exePath)
if ($null -eq $embeddedIcon) {
    throw "Windows executable has no associated app icon: $exePath"
}

$expectedBitmap = $expectedIcon.ToBitmap()
$embeddedBitmap = $embeddedIcon.ToBitmap()
try {
    if ($expectedBitmap.Size -ne $embeddedBitmap.Size) {
        throw "Windows executable icon dimensions do not match the Jev-Switch icon: $exePath"
    }

    for ($y = 0; $y -lt $expectedBitmap.Height; $y++) {
        for ($x = 0; $x -lt $expectedBitmap.Width; $x++) {
            if ($expectedBitmap.GetPixel($x, $y).ToArgb() -ne $embeddedBitmap.GetPixel($x, $y).ToArgb()) {
                throw "Windows executable icon does not match src-tauri/icons/icon.ico: $exePath"
            }
        }
    }
}
finally {
    $expectedBitmap.Dispose()
    $embeddedBitmap.Dispose()
    $expectedIcon.Dispose()
    $embeddedIcon.Dispose()
}

Write-Host "Verified Jev-Switch Windows app icon: $exePath"
