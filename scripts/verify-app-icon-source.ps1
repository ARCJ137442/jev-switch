param(
    [string]$IconPath = (Join-Path $PSScriptRoot '../src-tauri/app-icon.png'),
    [string]$GeneratedPngPath = (Join-Path $PSScriptRoot '../src-tauri/icons/icon.png'),
    [string]$IcoPath = (Join-Path $PSScriptRoot '../src-tauri/icons/icon.ico')
)

$ErrorActionPreference = 'Stop'
$approvedJIconSha256 = '7F20C3297D5E8C5478A7B9206740D24029CEBB9A1E9988DA571A8AFB0476CE58'

function Assert-MonochromeBitmap([System.Drawing.Bitmap]$Bitmap, [string]$Description) {
    for ($y = 0; $y -lt $Bitmap.Height; $y++) {
        for ($x = 0; $x -lt $Bitmap.Width; $x++) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if ($pixel.A -gt 8 -and ($pixel.R -ne $pixel.G -or $pixel.G -ne $pixel.B)) {
                throw "$Description contains a colored pixel at ($x, $y). The Jev-Switch app mark is the black-and-white J."
            }
        }
    }
}

$sourcePath = (Resolve-Path -LiteralPath $IconPath).Path
$sourceHash = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash
if ($sourceHash -ne $approvedJIconSha256) {
    throw "The canonical app icon is not the approved black-and-white J master. SHA-256=$sourceHash"
}

Add-Type -AssemblyName System.Drawing
$sourceBitmap = [System.Drawing.Bitmap]::FromFile($sourcePath)
try {
    if ($sourceBitmap.Width -ne 512 -or $sourceBitmap.Height -ne 512) {
        throw 'The canonical black-and-white J master must be 512x512 pixels.'
    }
    Assert-MonochromeBitmap $sourceBitmap 'The canonical app icon'
}
finally {
    $sourceBitmap.Dispose()
}

$generatedPath = (Resolve-Path -LiteralPath $GeneratedPngPath).Path
$generatedBitmap = [System.Drawing.Bitmap]::FromFile($generatedPath)
$sourceBitmap = [System.Drawing.Bitmap]::FromFile($sourcePath)
try {
    if ($generatedBitmap.Size -ne $sourceBitmap.Size) {
        throw 'src-tauri/icons/icon.png must use the same 512x512 canvas as app-icon.png.'
    }
    Assert-MonochromeBitmap $generatedBitmap 'src-tauri/icons/icon.png'
    for ($y = 0; $y -lt $sourceBitmap.Height; $y++) {
        for ($x = 0; $x -lt $sourceBitmap.Width; $x++) {
            if ($sourceBitmap.GetPixel($x, $y).ToArgb() -ne $generatedBitmap.GetPixel($x, $y).ToArgb()) {
                throw "src-tauri/icons/icon.png differs from app-icon.png at ($x, $y). Regenerate platform resources from the canonical master."
            }
        }
    }
}
finally {
    $sourceBitmap.Dispose()
    $generatedBitmap.Dispose()
}

$icoBytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $IcoPath).Path)
if ($icoBytes.Length -lt 22 -or [BitConverter]::ToUInt16($icoBytes, 0) -ne 0 -or [BitConverter]::ToUInt16($icoBytes, 2) -ne 1) {
    throw 'The Windows app ICO has an invalid header.'
}
$frameCount = [BitConverter]::ToUInt16($icoBytes, 4)
if ($frameCount -lt 6) {
    throw "The Windows app ICO contains only $frameCount frame(s); expected the standard small and large icon sizes."
}
$requiredSizes = @(16, 24, 32, 48, 64, 256)
$availableSizes = [Collections.Generic.HashSet[int]]::new()
for ($index = 0; $index -lt $frameCount; $index++) {
    $entryOffset = 6 + ($index * 16)
    $width = if ($icoBytes[$entryOffset] -eq 0) { 256 } else { [int]$icoBytes[$entryOffset] }
    $height = if ($icoBytes[$entryOffset + 1] -eq 0) { 256 } else { [int]$icoBytes[$entryOffset + 1] }
    if ($width -ne $height) { throw "ICO frame $index is not square ($width x $height)." }
    [void]$availableSizes.Add($width)
    $imageLength = [BitConverter]::ToUInt32($icoBytes, $entryOffset + 8)
    $imageOffset = [BitConverter]::ToUInt32($icoBytes, $entryOffset + 12)
    if ($imageLength -eq 0 -or $imageOffset + $imageLength -gt $icoBytes.Length) {
        throw "ICO frame $index has an invalid image range."
    }

    $frameStream = [IO.MemoryStream]::new()
    $writer = [IO.BinaryWriter]::new($frameStream)
    $writer.Write([UInt16]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]1)
    $writer.Write($icoBytes, $entryOffset, 8)
    $writer.Write([UInt32]$imageLength)
    $writer.Write([UInt32]22)
    $writer.Write($icoBytes, [int]$imageOffset, [int]$imageLength)
    $writer.Flush()
    $frameStream.Position = 0
    try {
        $frameIcon = [System.Drawing.Icon]::new($frameStream)
        $frameBitmap = $frameIcon.ToBitmap()
        try {
            if ($frameBitmap.Width -ne $width -or $frameBitmap.Height -ne $height) {
                throw "ICO frame $index declared as $width x $height but decodes as $($frameBitmap.Width) x $($frameBitmap.Height)."
            }
            Assert-MonochromeBitmap $frameBitmap "ICO frame $width x $height"
        }
        finally {
            $frameBitmap.Dispose()
            $frameIcon.Dispose()
        }
    }
    finally {
        $writer.Dispose()
        $frameStream.Dispose()
    }
}
foreach ($size in $requiredSizes) {
    if (-not $availableSizes.Contains($size)) {
        throw "The Windows app ICO is missing its $size x $size frame."
    }
}

Write-Host "Verified approved black-and-white J icon source: $sourceHash"
Write-Host "Verified monochrome ICO frames: $((@($availableSizes) | Sort-Object -Unique) -join ', ')"
