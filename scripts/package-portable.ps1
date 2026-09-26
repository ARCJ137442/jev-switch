param(
    [Parameter(Mandatory = $true)]
    [string]$Destination,
    [string]$MsiPath,
    [string]$NsisPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$tauriConfig = Get-Content -LiteralPath (Join-Path $repoRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

$shellPath = Join-Path $repoRoot 'src-tauri/target/release/jev-switch.exe'
$daemonPath = Join-Path $repoRoot 'src-tauri/target/release/jev-switch-daemon.exe'
$daemonBuildPath = Join-Path $repoRoot 'rs/target/release/jev-switch.exe'
$uiDistPath = Join-Path $repoRoot 'ui/dist'

foreach ($required in @($shellPath, $daemonPath, $daemonBuildPath, (Join-Path $uiDistPath 'index.html'))) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required release input is missing: $required"
    }
}

if ((Get-Sha256 $daemonPath) -ne (Get-Sha256 $daemonBuildPath)) {
    throw 'The Tauri sidecar and current Rust release daemon hashes differ; rebuild the sidecar before packaging.'
}

$html = Get-Content -LiteralPath (Join-Path $uiDistPath 'index.html') -Raw
$assetRelativePaths = @()
foreach ($asset in [regex]::Matches($html, '(?:src|href)="([^"]+\.(?:js|css))"')) {
    $relativeAssetPath = $asset.Groups[1].Value -replace '/', '\'
    $assetPath = Join-Path $uiDistPath $relativeAssetPath
    if (-not (Test-Path -LiteralPath $assetPath -PathType Leaf)) {
        throw "The UI build references a missing asset: $assetPath"
    }
    $assetRelativePaths += $relativeAssetPath
}

if (-not [IO.Path]::IsPathRooted($Destination)) {
    $Destination = Join-Path $repoRoot $Destination
}
$destinationPath = [IO.Path]::GetFullPath($Destination)
if (Test-Path -LiteralPath $destinationPath) {
    throw "Destination already exists; choose a new empty path: $destinationPath"
}

New-Item -ItemType Directory -Path $destinationPath | Out-Null
New-Item -ItemType Directory -Path (Join-Path $destinationPath 'resources/ui') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $destinationPath 'ui') -Force | Out-Null

Copy-Item -LiteralPath $shellPath -Destination (Join-Path $destinationPath 'jev-switch.exe')
Copy-Item -LiteralPath $daemonPath -Destination (Join-Path $destinationPath 'jev-switch-daemon.exe')
Copy-Item -LiteralPath $daemonPath -Destination (Join-Path $destinationPath 'resources/jev-switch-daemon.exe')
Copy-Item -LiteralPath $uiDistPath -Destination (Join-Path $destinationPath 'ui') -Recurse
Copy-Item -LiteralPath $uiDistPath -Destination (Join-Path $destinationPath 'resources/ui') -Recurse

$fileHashes = [ordered]@{}
foreach ($relative in @(
    'jev-switch.exe',
    'jev-switch-daemon.exe',
    'resources/jev-switch-daemon.exe',
    'ui/dist/index.html'
)) {
    $outputPath = Join-Path $destinationPath $relative
    $sourcePath = switch ($relative) {
        'jev-switch.exe' { $shellPath }
        { $_ -like '*daemon.exe' } { $daemonPath; break }
        default { Join-Path $uiDistPath 'index.html' }
    }
    $actualHash = Get-Sha256 $outputPath
    if ($actualHash -ne (Get-Sha256 $sourcePath)) {
        throw "Portable file hash mismatch after copy: $outputPath"
    }
    $fileHashes[$relative.Replace('\', '/')] = $actualHash
}

$assetHashes = [ordered]@{}
foreach ($relativeAssetPath in $assetRelativePaths) {
    $sourceAsset = Join-Path $uiDistPath $relativeAssetPath
    $portableAsset = Join-Path (Join-Path $destinationPath 'ui/dist') $relativeAssetPath
    $resourceAsset = Join-Path (Join-Path $destinationPath 'resources/ui/dist') $relativeAssetPath
    $sourceHash = Get-Sha256 $sourceAsset
    if ((Get-Sha256 $portableAsset) -ne $sourceHash -or (Get-Sha256 $resourceAsset) -ne $sourceHash) {
        throw "Portable UI asset hash mismatch after copy: $relativeAssetPath"
    }
    $assetHashes[$relativeAssetPath.Replace('\', '/')] = $sourceHash
}

$msiInputHashes = $null
if ($MsiPath) {
    if (-not (Test-Path -LiteralPath $MsiPath -PathType Leaf)) {
        throw "Installer artifact is missing: $MsiPath"
    }
    $wixManifestPath = Join-Path $repoRoot 'src-tauri/target/release/wix/x64/main.wxs'
    if (-not (Test-Path -LiteralPath $wixManifestPath -PathType Leaf)) {
        throw "The MSI WiX source manifest is missing: $wixManifestPath"
    }
    $wixManifest = Get-Content -LiteralPath $wixManifestPath -Raw
    $msiInputSources = [ordered]@{
        shell = 'File Id="Path" Source="([^"]*jev-switch\.exe)"'
        daemon = 'File Id="Bin_jev_switch_daemon\.exe" Source="([^"]+)"'
        ui_index = 'Source="([^"]*ui[\\/]dist[\\/]index\.html)"'
        ui_js = 'Source="([^"]*index-[^"\\/]+\.js)"'
        ui_css = 'Source="([^"]*index-[^"\\/]+\.css)"'
    }
    $portableExpectedHashes = @{
        shell = Get-Sha256 (Join-Path $destinationPath 'jev-switch.exe')
        daemon = Get-Sha256 (Join-Path $destinationPath 'jev-switch-daemon.exe')
        ui_index = Get-Sha256 (Join-Path $destinationPath 'ui/dist/index.html')
        ui_js = $null
        ui_css = $null
    }
    foreach ($relativeAssetPath in $assetRelativePaths) {
        if ($relativeAssetPath -match '\.js$') { $portableExpectedHashes.ui_js = $assetHashes[$relativeAssetPath.Replace('\', '/')] }
        if ($relativeAssetPath -match '\.css$') { $portableExpectedHashes.ui_css = $assetHashes[$relativeAssetPath.Replace('\', '/')] }
    }
    $msiInputHashes = [ordered]@{}
    foreach ($label in $msiInputSources.Keys) {
        $match = [regex]::Match($wixManifest, $msiInputSources[$label])
        if (-not $match.Success) {
            throw "The MSI WiX manifest does not identify its $label input."
        }
        $sourcePath = $match.Groups[1].Value
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "The MSI $label input recorded by WiX is missing: $sourcePath"
        }
        $sourceHash = Get-Sha256 $sourcePath
        if (-not $portableExpectedHashes[$label] -or $sourceHash -ne $portableExpectedHashes[$label]) {
            throw "The MSI $label input does not match the portable build input."
        }
        $msiInputHashes[$label] = $sourceHash
    }
}

$runInstructions = @'
Jev-Switch portable test runtime

Run: double-click jev-switch.exe in this directory.
Keep jev-switch-daemon.exe and the resources/ui/dist directory beside it. This is a folder-based portable runtime, not a standalone single-file executable.

It uses the existing %APPDATA%\jev-switch configuration and database and listens on 127.0.0.1:11435. Before launch, make sure no other Jev-Switch daemon is already using that port; the shell reuses a compatible daemon it finds there.

This build does not run an installer. The portable files come from the same Tauri build inputs as the MSI candidate. Complete a cold launch and real route-trace check before treating runtime acceptance as complete.
'@
Set-Content -LiteralPath (Join-Path $destinationPath 'RUN-PORTABLE.txt') -Value $runInstructions -Encoding UTF8

$manifest = [ordered]@{
    product = 'jev-switch'
    version = $tauriConfig.version
    layout = 'folder-portable'
    built_at = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    shell_sha256 = Get-Sha256 $shellPath
    daemon_sha256 = Get-Sha256 $daemonPath
    ui_index_sha256 = Get-Sha256 (Join-Path $uiDistPath 'index.html')
    files = $fileHashes
    ui_assets = $assetHashes
}
if ($msiInputHashes) {
    $manifest['msi_payload_inputs'] = $msiInputHashes
}
if ($MsiPath -or $NsisPath) {
    $installerHashes = [ordered]@{}
    foreach ($installerPath in @($MsiPath, $NsisPath) | Where-Object { $_ }) {
        if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
            throw "Installer artifact is missing: $installerPath"
        }
        $installerHashes[[IO.Path]::GetFileName($installerPath)] = Get-Sha256 $installerPath
    }
    $manifest['installers'] = $installerHashes
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $destinationPath 'build-manifest.json') -Encoding UTF8

Write-Output "Portable runtime created: $destinationPath"
Write-Output ("Shell SHA-256:  {0}" -f $manifest.shell_sha256)
Write-Output ("Daemon SHA-256: {0}" -f $manifest.daemon_sha256)
