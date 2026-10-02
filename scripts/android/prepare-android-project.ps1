param(
    [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
    [string]$AndroidProject = 'src-tauri/gen/android'
)

$ErrorActionPreference = 'Stop'
$androidDisplayName = 'Jev Switch'

function Assert-MonochromePng {
    param(
        [string]$Path,
        [string]$Description
    )

    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::FromFile((Resolve-Path -LiteralPath $Path).Path)
    try {
        for ($y = 0; $y -lt $bitmap.Height; $y++) {
            for ($x = 0; $x -lt $bitmap.Width; $x++) {
                $pixel = $bitmap.GetPixel($x, $y)
                if ($pixel.A -gt 8 -and ($pixel.R -ne $pixel.G -or $pixel.G -ne $pixel.B)) {
                    throw "$Description contains a colored pixel at ($x, $y)."
                }
            }
        }
    }
    finally {
        $bitmap.Dispose()
    }
}

function Get-PortableRelativePath {
    param(
        [string]$BasePath,
        [string]$Path
    )

    # Windows PowerShell 5.1 lacks System.IO.Path.GetRelativePath; use URI
    # semantics so local CI and the maintained PowerShell 7 path agree.
    $baseFullPath = [System.IO.Path]::GetFullPath($BasePath).TrimEnd('\') + '\'
    $pathFullPath = [System.IO.Path]::GetFullPath($Path)
    $baseUri = [Uri]::new($baseFullPath)
    $pathUri = [Uri]::new($pathFullPath)
    return [Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace('/', '\')
}

function Get-AndroidVersionCode {
    param([string]$Version)

    if ($Version -notmatch '^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$') {
        throw "Android release requires a SemVer version, received '$Version'."
    }

    $major = [int64]$Matches[1]
    $minor = [int64]$Matches[2]
    $patch = [int64]$Matches[3]
    if ($minor -ge 1000 -or $patch -ge 1000) {
        throw "Android versionCode encoding supports minor and patch values below 1000: '$Version'."
    }

    $code = ($major * 1000000) + ($minor * 1000) + $patch
    if ($code -lt 1 -or $code -gt 2100000000) {
        throw "Android versionCode is outside the supported range for '$Version'."
    }
    return $code
}

$tauriRoot = Join-Path $ProjectRoot 'src-tauri'
$androidRoot = Join-Path $ProjectRoot $AndroidProject
$appIcon = Join-Path $tauriRoot 'app-icon.png'
$keepaliveSource = Join-Path $tauriRoot 'android/keepalive/src/main'
$resRoot = Join-Path $androidRoot 'app/src/main/res'
$mainSourceRoot = Join-Path $androidRoot 'app/src/main'
$buildFile = Join-Path $androidRoot 'app/build.gradle.kts'
$propertiesFile = Join-Path $androidRoot 'app/tauri.properties'
$taskIconPath = Join-Path $resRoot 'drawable-nodpi/ic_task.png'

if (-not (Test-Path -LiteralPath $appIcon -PathType Leaf)) {
    throw "Canonical Jev-Switch icon is missing: $appIcon"
}
& (Join-Path $PSScriptRoot '../verify-app-icon-source.ps1')
if (-not (Test-Path -LiteralPath $keepaliveSource -PathType Container)) {
    throw "Android keepalive sources are missing: $keepaliveSource"
}
if (-not (Test-Path -LiteralPath $resRoot -PathType Container) -or -not (Test-Path -LiteralPath $buildFile -PathType Leaf)) {
    throw "Tauri Android project is not initialized: $androidRoot"
}

# Tauri's generated release variant disables cleartext by default. The embedded
# gateway and user-selected LAN services use HTTP, so a signed APK would bind
# 127.0.0.1 successfully while Android WebView rejects every API fetch.
$gradleSource = [IO.File]::ReadAllText($buildFile)
$cleartextDefault = 'manifestPlaceholders["usesCleartextTraffic"] = "false"'
if ($gradleSource.Contains($cleartextDefault)) {
    $gradleSource = $gradleSource.Replace($cleartextDefault, 'manifestPlaceholders["usesCleartextTraffic"] = "true"')
    [IO.File]::WriteAllText($buildFile, $gradleSource, [Text.UTF8Encoding]::new($false))
}
if ($gradleSource -match 'manifestPlaceholders\["usesCleartextTraffic"\] = "false"' -or
    $gradleSource -notmatch 'defaultConfig\s*\{\s*manifestPlaceholders\["usesCleartextTraffic"\] = "true"') {
    throw 'Android release cleartext setting is not enabled for the local HTTP gateway.'
}

$generatedSources = Join-Path $mainSourceRoot 'java/io/github/arcj137442/jevswitch/keepalive'
New-Item -ItemType Directory -Path $generatedSources -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $keepaliveSource 'java') -Filter '*.kt' -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $generatedSources $_.Name) -Force
}

$mainActivitySource = Join-Path $tauriRoot 'android/main/MainActivity.kt'
$generatedActivity = Join-Path $mainSourceRoot 'java/io/github/arcj137442/jevswitch/MainActivity.kt'
if (-not (Test-Path -LiteralPath $mainActivitySource -PathType Leaf)) {
    throw "Maintained Android MainActivity source is missing: $mainActivitySource"
}
Copy-Item -LiteralPath $mainActivitySource -Destination $generatedActivity -Force
Get-ChildItem -LiteralPath (Join-Path $keepaliveSource 'res') -File -Recurse | ForEach-Object {
    $relative = Get-PortableRelativePath -BasePath (Join-Path $keepaliveSource 'res') -Path $_.FullName
    $destination = Join-Path $resRoot $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
}

# MainActivity uses this opaque raster for recent tasks. Keeping it separate
# from adaptive-icon layers avoids Android inheriting a generated template icon.
New-Item -ItemType Directory -Path (Split-Path -Parent $taskIconPath) -Force | Out-Null
Copy-Item -LiteralPath $appIcon -Destination $taskIconPath -Force
if ((Get-FileHash -LiteralPath $taskIconPath -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $appIcon -Algorithm SHA256).Hash) {
    throw 'Android recent-task icon does not match the canonical Jev Switch icon.'
}

$manifestPath = Join-Path $mainSourceRoot 'AndroidManifest.xml'
[xml]$manifest = Get-Content -LiteralPath $manifestPath -Raw
$androidNamespace = 'http://schemas.android.com/apk/res/android'
$namespaces = [System.Xml.XmlNamespaceManager]::new($manifest.NameTable)
$namespaces.AddNamespace('android', $androidNamespace)
$root = $manifest.DocumentElement
foreach ($permission in @(
    'android.permission.FOREGROUND_SERVICE',
    'android.permission.FOREGROUND_SERVICE_SPECIAL_USE',
    'android.permission.POST_NOTIFICATIONS'
)) {
    $existing = $root.SelectSingleNode("uses-permission[@android:name='$permission']", $namespaces)
    if ($null -eq $existing) {
        $node = $manifest.CreateElement('uses-permission')
        $node.SetAttribute('name', $androidNamespace, $permission)
        [void]$root.InsertBefore($node, $root.SelectSingleNode('application'))
    }
}
$application = $root.SelectSingleNode('application')
[void]$application.SetAttribute('label', $androidNamespace, '@string/app_name')
$activity = $application.SelectSingleNode("activity[@android:name='.MainActivity']", $namespaces)
if ($null -eq $activity) {
    throw 'Generated Android MainActivity is missing from the manifest.'
}
[void]$activity.SetAttribute('icon', $androidNamespace, '@mipmap/ic_launcher')
[void]$activity.SetAttribute('roundIcon', $androidNamespace, '@mipmap/ic_launcher_round')
[void]$activity.SetAttribute('label', $androidNamespace, '@string/main_activity_title')
$service = $application.SelectSingleNode("service[@android:name='.keepalive.JevKeepaliveService']", $namespaces)
if ($null -eq $service) {
    $service = $manifest.CreateElement('service')
    $service.SetAttribute('name', $androidNamespace, '.keepalive.JevKeepaliveService')
    [void]$application.AppendChild($service)
}
[void]$service.SetAttribute('exported', $androidNamespace, 'false')
[void]$service.SetAttribute('stopWithTask', $androidNamespace, 'false')
[void]$service.SetAttribute('foregroundServiceType', $androidNamespace, 'specialUse')
$subtype = $service.SelectSingleNode("property[@android:name='android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE']", $namespaces)
if ($null -eq $subtype) {
    $subtype = $manifest.CreateElement('property')
    $subtype.SetAttribute('name', $androidNamespace, 'android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE')
    [void]$service.AppendChild($subtype)
}
[void]$subtype.SetAttribute('value', $androidNamespace, 'Keep the explicitly enabled local Jev Switch gateway available while the app is in the background.')

$tileService = $application.SelectSingleNode("service[@android:name='.keepalive.JevGatewayTileService']", $namespaces)
if ($null -eq $tileService) {
    $tileService = $manifest.CreateElement('service')
    $tileService.SetAttribute('name', $androidNamespace, '.keepalive.JevGatewayTileService')
    [void]$application.AppendChild($tileService)
}
[void]$tileService.SetAttribute('label', $androidNamespace, '@string/keepalive_tile_name')
[void]$tileService.SetAttribute('icon', $androidNamespace, '@mipmap/ic_launcher')
[void]$tileService.SetAttribute('permission', $androidNamespace, 'android.permission.BIND_QUICK_SETTINGS_TILE')
[void]$tileService.SetAttribute('exported', $androidNamespace, 'true')
$tileFilter = $tileService.SelectSingleNode('intent-filter')
if ($null -eq $tileFilter) {
    $tileFilter = $manifest.CreateElement('intent-filter')
    $tileAction = $manifest.CreateElement('action')
    $tileAction.SetAttribute('name', $androidNamespace, 'android.service.quicksettings.action.QS_TILE')
    [void]$tileFilter.AppendChild($tileAction)
    [void]$tileService.AppendChild($tileFilter)
}
$activeTile = $tileService.SelectSingleNode("meta-data[@android:name='android.service.quicksettings.ACTIVE_TILE']", $namespaces)
if ($null -eq $activeTile) {
    $activeTile = $manifest.CreateElement('meta-data')
    $activeTile.SetAttribute('name', $androidNamespace, 'android.service.quicksettings.ACTIVE_TILE')
    [void]$tileService.AppendChild($activeTile)
}
[void]$activeTile.SetAttribute('value', $androidNamespace, 'true')
$fileProvider = $application.SelectSingleNode("provider[@android:name='androidx.core.content.FileProvider']", $namespaces)
if ($null -eq $fileProvider) {
    $fileProvider = $manifest.CreateElement('provider')
    $fileProvider.SetAttribute('name', $androidNamespace, 'androidx.core.content.FileProvider')
    [void]$application.AppendChild($fileProvider)
}
[void]$fileProvider.SetAttribute('authorities', $androidNamespace, '${applicationId}.fileprovider')
[void]$fileProvider.SetAttribute('exported', $androidNamespace, 'false')
[void]$fileProvider.SetAttribute('grantUriPermissions', $androidNamespace, 'true')
$filePaths = $fileProvider.SelectSingleNode("meta-data[@android:name='android.support.FILE_PROVIDER_PATHS']", $namespaces)
if ($null -eq $filePaths) {
    $filePaths = $manifest.CreateElement('meta-data')
    $filePaths.SetAttribute('name', $androidNamespace, 'android.support.FILE_PROVIDER_PATHS')
    [void]$fileProvider.AppendChild($filePaths)
}
[void]$filePaths.SetAttribute('resource', $androidNamespace, '@xml/jev_debug_file_paths')
$manifest.Save($manifestPath)

$stringsPath = Join-Path $resRoot 'values/strings.xml'
if (-not (Test-Path -LiteralPath $stringsPath -PathType Leaf)) {
    throw "Generated Android strings resource is missing: $stringsPath"
}
[xml]$strings = Get-Content -LiteralPath $stringsPath -Raw
$resources = $strings.SelectSingleNode('/resources')
if ($null -eq $resources) {
    throw "Generated Android strings resource has no <resources> root: $stringsPath"
}
foreach ($stringSpec in @(
    @{ Name = 'app_name'; Value = $androidDisplayName },
    @{ Name = 'main_activity_title'; Value = $androidDisplayName }
)) {
    $stringNode = $resources.SelectSingleNode("string[@name='$($stringSpec.Name)']")
    if ($null -eq $stringNode) {
        $stringNode = $strings.CreateElement('string')
        $stringNode.SetAttribute('name', $stringSpec.Name)
        [void]$resources.AppendChild($stringNode)
    }
    $stringNode.InnerText = $stringSpec.Value
}
$strings.Save($stringsPath)

$package = Get-Content -LiteralPath (Join-Path $ProjectRoot 'ui/package.json') -Raw | ConvertFrom-Json
$tauriConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.conf.json') -Raw | ConvertFrom-Json
$androidConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.android.conf.json') -Raw | ConvertFrom-Json
$version = [string]$package.version
if ($tauriConfig.version -ne $version -or $androidConfig.version -ne $version) {
    throw "Android, Tauri, and UI versions must agree. UI=$version, Tauri=$($tauriConfig.version), Android=$($androidConfig.version)."
}
if ([string]$androidConfig.productName -ne $androidDisplayName) {
    throw "Android productName must be '$androidDisplayName'; internal ids remain 'jev-switch'."
}
$versionCode = Get-AndroidVersionCode -Version $version

$iconOutput = Join-Path ([IO.Path]::GetTempPath()) ("jev-switch-icons-{0}" -f [Guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $iconOutput -Force | Out-Null
    Push-Location $ProjectRoot
    try {
        & cargo tauri icon $appIcon --output $iconOutput
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri icon generation failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }

    $generatedAndroid = Join-Path $iconOutput 'android'
    if (-not (Test-Path -LiteralPath $generatedAndroid -PathType Container)) {
        throw "Tauri did not generate Android icon resources: $generatedAndroid"
    }

    $copied = 0
    Get-ChildItem -LiteralPath $generatedAndroid -File -Recurse | ForEach-Object {
        $relativePath = Get-PortableRelativePath -BasePath $generatedAndroid -Path $_.FullName
        $destination = Join-Path $resRoot $relativePath
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
        $copied++
    }

    if ($copied -lt 15) {
        throw "Tauri generated only $copied Android icon files; expected all launcher densities."
    }

    # Tauri Android init leaves template resources in the generated tree. They
    # are not part of the canonical output and can otherwise be selected by a
    # density/API fallback, so remove them before compiling the APK.
    foreach ($stalePath in @(
        (Join-Path $resRoot 'drawable/ic_launcher_background.xml'),
        (Join-Path $resRoot 'drawable-v24/ic_launcher_foreground.xml')
    )) {
        if (Test-Path -LiteralPath $stalePath) {
            Remove-Item -LiteralPath $stalePath -Force
        }
    }

    $launcherPngs = @(Get-ChildItem -LiteralPath $resRoot -File -Recurse | Where-Object {
        $_.Name -in @('ic_launcher.png', 'ic_launcher_round.png', 'ic_launcher_foreground.png')
    })
    if ($launcherPngs.Count -lt 15) {
        throw "Android launcher output contains only $($launcherPngs.Count) raster resources."
    }
    foreach ($launcherPng in $launcherPngs) {
        Assert-MonochromePng -Path $launcherPng.FullName -Description "Android launcher icon $($launcherPng.FullName)"
    }
}
finally {
    if (Test-Path -LiteralPath $iconOutput) {
        Remove-Item -LiteralPath $iconOutput -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$properties = @()
if (Test-Path -LiteralPath $propertiesFile -PathType Leaf) {
    $properties = @(Get-Content -LiteralPath $propertiesFile | Where-Object {
        $_ -notmatch '^\s*tauri\.android\.version(Code|Name)\s*='
    })
}
$properties += "tauri.android.versionCode=$versionCode"
$properties += "tauri.android.versionName=$version"
[IO.File]::WriteAllLines($propertiesFile, [string[]]$properties, [Text.UTF8Encoding]::new($false))

Write-Host "Android version: $version (versionCode $versionCode)"
Write-Host "Android launcher resources synced from app-icon.png ($copied files)."
Write-Host "Android user-facing name: $androidDisplayName; internal package/binary id remains jev-switch."
Write-Host 'Android recent-task icon and stale template icon cleanup verified.'
Write-Host 'Android foreground keepalive service and Quick Settings tile synced.'
Write-Host 'Android release permits HTTP for the embedded loopback gateway and user-selected LAN APIs.'
