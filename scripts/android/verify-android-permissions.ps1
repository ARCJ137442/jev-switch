param(
    [Parameter(Mandatory = $true)] [string]$ManifestPath,
    [Parameter(Mandatory = $true)] [string]$ResourceRoot,
    [switch]$SourceTemplate,
    [string]$ProjectRoot
)

$ErrorActionPreference = 'Stop'
if (-not $ProjectRoot) { $ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path }
$androidNamespace = 'http://schemas.android.com/apk/res/android'
$appId = (Get-Content -LiteralPath (Join-Path $ProjectRoot 'src-tauri/tauri.android.conf.json') -Raw | ConvertFrom-Json).identifier
[xml]$manifest = Get-Content -LiteralPath $ManifestPath -Raw
$namespaces = [System.Xml.XmlNamespaceManager]::new($manifest.NameTable)
$namespaces.AddNamespace('android', $androidNamespace)
$root = $manifest.DocumentElement
$application = $root.SelectSingleNode('application')
if ($null -eq $application) { throw 'Android manifest has no application element.' }

foreach ($permission in @(
    'android.permission.INTERNET',
    'android.permission.FOREGROUND_SERVICE',
    'android.permission.FOREGROUND_SERVICE_SPECIAL_USE',
    'android.permission.POST_NOTIFICATIONS'
)) {
    if ($null -eq $root.SelectSingleNode("uses-permission[@android:name='$permission']", $namespaces)) {
        throw "Android manifest is missing $permission."
    }
}
foreach ($permission in @(
    'android.permission.READ_EXTERNAL_STORAGE',
    'android.permission.WRITE_EXTERNAL_STORAGE',
    'android.permission.MANAGE_EXTERNAL_STORAGE',
    'android.permission.QUERY_ALL_PACKAGES'
)) {
    if ($null -ne $root.SelectSingleNode("uses-permission[@android:name='$permission']", $namespaces)) {
        throw "Android manifest requests unnecessary broad access: $permission."
    }
}
$expectedCleartext = if ($SourceTemplate) { '${usesCleartextTraffic}' } else { 'true' }
if ($application.GetAttribute('usesCleartextTraffic', $androidNamespace) -ne $expectedCleartext) {
    throw 'Android release manifest blocks the embedded HTTP gateway.'
}

$provider = $application.SelectSingleNode("provider[@android:name='androidx.core.content.FileProvider']", $namespaces)
$expectedAuthority = if ($SourceTemplate) { '${applicationId}.fileprovider' } else { "$appId.fileprovider" }
if ($null -eq $provider -or
    $provider.GetAttribute('authorities', $androidNamespace) -ne $expectedAuthority -or
    $provider.GetAttribute('exported', $androidNamespace) -ne 'false' -or
    $provider.GetAttribute('grantUriPermissions', $androidNamespace) -ne 'true') {
    throw 'Android FileProvider is missing or incorrectly scoped.'
}
$pathsMetadata = $provider.SelectSingleNode("meta-data[@android:name='android.support.FILE_PROVIDER_PATHS']", $namespaces)
if ($null -eq $pathsMetadata -or $pathsMetadata.GetAttribute('resource', $androidNamespace) -ne '@xml/jev_debug_file_paths') {
    throw 'Android FileProvider has no debug-log paths resource.'
}
$pathsFile = Join-Path $ResourceRoot 'xml/jev_debug_file_paths.xml'
[xml]$paths = Get-Content -LiteralPath $pathsFile -Raw
$cachePath = $paths.SelectSingleNode("/paths/cache-path[@name='shared_debug_logs']")
if ($null -eq $cachePath -or $cachePath.GetAttribute('path') -ne 'shared_debug_logs/') {
    throw 'Android FileProvider does not limit sharing to the debug-log cache.'
}
$exportPath = $paths.SelectSingleNode("/paths/cache-path[@name='shared_exports']")
if ($null -eq $exportPath -or $exportPath.GetAttribute('path') -ne 'shared_exports/') {
    throw 'Android FileProvider does not limit JSON sharing to the export cache.'
}

$service = $application.SelectNodes('service') | Where-Object {
    $_.GetAttribute('name', $androidNamespace) -like '*JevKeepaliveService'
} | Select-Object -First 1
if ($null -eq $service -or
    $service.GetAttribute('exported', $androidNamespace) -ne 'false' -or
    $service.GetAttribute('foregroundServiceType', $androidNamespace) -ne 'specialUse') {
    throw 'Android foreground gateway service is missing its type or is exported.'
}
$tile = $application.SelectNodes('service') | Where-Object {
    $_.GetAttribute('name', $androidNamespace) -like '*JevGatewayTileService'
} | Select-Object -First 1
if ($null -eq $tile -or
    $tile.GetAttribute('permission', $androidNamespace) -ne 'android.permission.BIND_QUICK_SETTINGS_TILE' -or
    $tile.GetAttribute('exported', $androidNamespace) -ne 'true') {
    throw 'Android Quick Settings tile is missing its system binding permission.'
}

Write-Host 'Android network, foreground-service, notification, tile, and scoped log-sharing manifest checks passed.'
