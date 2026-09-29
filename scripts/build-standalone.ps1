param(
    [Parameter(Mandatory = $true)]
    [string]$PortableDirectory,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$portableRoot = (Resolve-Path -LiteralPath $PortableDirectory).Path
$manifestPath = Join-Path $portableRoot 'build-manifest.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

if ($manifest.product -ne 'jev-switch' -or $manifest.layout -ne 'folder-portable') {
    throw 'Standalone input must be a Jev-Switch folder-portable build with a build manifest.'
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ManifestValue($Map, [string]$Name) {
    $property = $Map.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return [string]$property.Value
}

$daemonPath = Join-Path $portableRoot 'jev-switch-daemon.exe'
$uiRoot = Join-Path $portableRoot 'ui/dist'
$uiIndexPath = Join-Path $uiRoot 'index.html'
foreach ($required in @($daemonPath, $uiIndexPath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Standalone input is missing a required runtime file: $required"
    }
}

if ((Get-Sha256 $daemonPath) -ne $manifest.daemon_sha256) {
    throw 'The standalone daemon does not match the portable build manifest.'
}
if ((Get-Sha256 $uiIndexPath) -ne $manifest.ui_index_sha256) {
    throw 'The standalone UI index does not match the portable build manifest.'
}

$uiFileHashes = $manifest.ui_files
if ($null -eq $uiFileHashes) {
    throw 'Portable input predates per-file UI hashes; rebuild it with the current package-portable.ps1.'
}
$actualUiFiles = @(Get-ChildItem -LiteralPath $uiRoot -Recurse -File)
$expectedUiFiles = @($uiFileHashes.PSObject.Properties)
if ($actualUiFiles.Count -ne $expectedUiFiles.Count) {
    throw 'The standalone UI file list does not match the portable build manifest.'
}
foreach ($file in $actualUiFiles) {
    $relativePath = $file.FullName.Substring($uiRoot.Length).TrimStart([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar).Replace('\', '/')
    $expectedHash = Get-ManifestValue $uiFileHashes $relativePath
    if (-not $expectedHash -or (Get-Sha256 $file.FullName) -ne $expectedHash) {
        throw "The standalone UI file does not match the portable build manifest: $relativePath"
    }
}

if (-not [IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $repoRoot $OutputPath
}
$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Path (Split-Path -Parent $outputFullPath) -Force | Out-Null

$stamp = [guid]::NewGuid().ToString('N')
$assetRoot = Join-Path (Join-Path $repoRoot 'src-tauri/target') "standalone-assets-$stamp"
${assetUiRoot} = Join-Path $assetRoot 'ui/dist'
New-Item -ItemType Directory -Path $assetUiRoot -Force | Out-Null

$shellCargo = Join-Path $repoRoot 'src-tauri/Cargo.toml'
$previousAssetRoot = $env:JEV_STANDALONE_ASSET_DIR
try {
    Copy-Item -LiteralPath $daemonPath -Destination (Join-Path $assetRoot 'jev-switch-daemon.exe')
    Get-ChildItem -LiteralPath $uiRoot -Force | Copy-Item -Destination $assetUiRoot -Recurse
    $env:JEV_STANDALONE_ASSET_DIR = $assetRoot
    Push-Location $repoRoot
    try {
        Write-Host 'Testing standalone resource validation and recovery...'
        & cargo test --locked --manifest-path $shellCargo --features standalone
        if ($LASTEXITCODE -ne 0) { throw "Standalone tests failed with exit code $LASTEXITCODE." }

        Write-Host 'Building the self-contained Windows desktop executable...'
        & cargo build --release --locked --manifest-path $shellCargo --features standalone
        if ($LASTEXITCODE -ne 0) { throw "Standalone build failed with exit code $LASTEXITCODE." }
    }
    finally {
        Pop-Location
    }

    $builtExe = Join-Path $repoRoot 'src-tauri/target/release/jev-switch.exe'
    if (-not (Test-Path -LiteralPath $builtExe -PathType Leaf)) {
        throw "Standalone build did not produce the expected executable: $builtExe"
    }
    Copy-Item -LiteralPath $builtExe -Destination $outputFullPath -Force

    Write-Output "Standalone executable: $outputFullPath"
    Write-Output "Standalone SHA-256: $(Get-Sha256 $outputFullPath)"
    Write-Output "Embedded daemon SHA-256: $(Get-Sha256 $daemonPath)"
    Write-Output "Embedded UI files: $($actualUiFiles.Count)"
}
finally {
    if ($null -eq $previousAssetRoot) {
        Remove-Item Env:JEV_STANDALONE_ASSET_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:JEV_STANDALONE_ASSET_DIR = $previousAssetRoot
    }
    if (Test-Path -LiteralPath $assetRoot) {
        Remove-Item -LiteralPath $assetRoot -Recurse -Force
    }
}
