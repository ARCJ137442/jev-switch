param(
    [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
    [string]$AndroidProject = 'src-tauri/gen/android'
)

$ErrorActionPreference = 'Stop'

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

$generatedSources = Join-Path $mainSourceRoot 'java/io/github/arcj137442/jevswitch/keepalive'
New-Item -ItemType Directory -Path $generatedSources -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $keepaliveSource 'java') -Filter '*.kt' -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $generatedSources $_.Name) -Force
}
Get-ChildItem -LiteralPath (Join-Path $keepaliveSource 'res') -File -Recurse | ForEach-Object {
    $relative = [IO.Path]::GetRelativePath((Join-Path $keepaliveSource 'res'), $_.FullName)
    $destination = Join-Path $resRoot $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
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
$service = $application.SelectSingleNode("service[@android:name='.keepalive.JevKeepaliveService']", $namespaces)
if ($null -eq $service) {
    $service = $manifest.CreateElement('service')
    $service.SetAttribute('name', $androidNamespace, '.keepalive.JevKeepaliveService')
    [void]$application.AppendChild($service)
}
$service.SetAttribute('exported', $androidNamespace, 'false')
$service.SetAttribute('stopWithTask', $androidNamespace, 'false')
$service.SetAttribute('foregroundServiceType', $androidNamespace, 'specialUse')
$subtype = $service.SelectSingleNode("property[@android:name='android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE']", $namespaces)
if ($null -eq $subtype) {
    $subtype = $manifest.CreateElement('property')
    $subtype.SetAttribute('name', $androidNamespace, 'android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE')
    [void]$service.AppendChild($subtype)
}
$subtype.SetAttribute('value', $androidNamespace, 'Keep the explicitly enabled local Jev-Switch gateway available while the app is in the background.')

$tileService = $application.SelectSingleNode("service[@android:name='.keepalive.JevGatewayTileService']", $namespaces)
if ($null -eq $tileService) {
    $tileService = $manifest.CreateElement('service')
    $tileService.SetAttribute('name', $androidNamespace, '.keepalive.JevGatewayTileService')
    [void]$application.AppendChild($tileService)
}
$tileService.SetAttribute('label', $androidNamespace, '@string/keepalive_tile_name')
$tileService.SetAttribute('icon', $androidNamespace, '@mipmap/ic_launcher')
$tileService.SetAttribute('permission', $androidNamespace, 'android.permission.BIND_QUICK_SETTINGS_TILE')
$tileService.SetAttribute('exported', $androidNamespace, 'true')
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
$activeTile.SetAttribute('value', $androidNamespace, 'true')
$manifest.Save($manifestPath)

$package = Get-Content -LiteralPath (Join-Path $ProjectRoot 'ui/package.json') -Raw | ConvertFrom-Json
$tauriConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.conf.json') -Raw | ConvertFrom-Json
$androidConfig = Get-Content -LiteralPath (Join-Path $tauriRoot 'tauri.android.conf.json') -Raw | ConvertFrom-Json
$version = [string]$package.version
if ($tauriConfig.version -ne $version -or $androidConfig.version -ne $version) {
    throw "Android, Tauri, and UI versions must agree. UI=$version, Tauri=$($tauriConfig.version), Android=$($androidConfig.version)."
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
        $relativePath = [IO.Path]::GetRelativePath($generatedAndroid, $_.FullName)
        $destination = Join-Path $resRoot $relativePath
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
        $copied++
    }

    if ($copied -lt 15) {
        throw "Tauri generated only $copied Android icon files; expected all launcher densities."
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
Write-Host 'Android foreground keepalive service and Quick Settings tile synced.'
