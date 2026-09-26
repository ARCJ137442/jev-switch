param(
    [string]$PortableDestination
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$tauriRoot = Join-Path $repoRoot 'src-tauri'
$targetTriple = 'x86_64-pc-windows-msvc'
$daemonReleasePath = Join-Path $repoRoot 'rs/target/release/jev-switch.exe'
$sidecarPath = Join-Path $tauriRoot "binaries/jev-switch-daemon-$targetTriple.exe"
$packageScript = Join-Path $PSScriptRoot 'package-portable.ps1'

Write-Host 'Building the Windows daemon release binary...'
Push-Location $repoRoot
try {
    & cargo build --release --manifest-path rs/Cargo.toml -p jev-switch-daemon --locked
    if ($LASTEXITCODE -ne 0) {
        throw "Daemon release build failed with exit code $LASTEXITCODE."
    }

    Copy-Item -LiteralPath $daemonReleasePath -Destination $sidecarPath -Force

    Write-Host 'Building MSI and NSIS from the current daemon and UI inputs...'
    Push-Location $tauriRoot
    try {
        & cargo tauri build --bundles nsis,msi
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri bundle build failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    Pop-Location
}

$msiArtifacts = @(Get-ChildItem -LiteralPath (Join-Path $tauriRoot 'target/release/bundle/msi') -Filter '*.msi' -File)
$nsisArtifacts = @(Get-ChildItem -LiteralPath (Join-Path $tauriRoot 'target/release/bundle/nsis') -Filter '*.exe' -File)
if ($msiArtifacts.Count -ne 1 -or $nsisArtifacts.Count -ne 1) {
    throw "Expected exactly one MSI and one NSIS executable; found MSI=$($msiArtifacts.Count), NSIS=$($nsisArtifacts.Count)."
}

if (-not $PortableDestination) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
    $portableParent = Join-Path $tauriRoot 'target/release/bundle/portable'
    New-Item -ItemType Directory -Path $portableParent -Force | Out-Null
    $PortableDestination = Join-Path $portableParent "jev-switch-$stamp"
}

Write-Host 'Packaging the no-installer runtime from these same release inputs...'
& $packageScript `
    -Destination $PortableDestination `
    -MsiPath $msiArtifacts[0].FullName `
    -NsisPath $nsisArtifacts[0].FullName

$portableManifestPath = Join-Path ([IO.Path]::GetFullPath($PortableDestination)) 'build-manifest.json'
$portableManifest = Get-Content -LiteralPath $portableManifestPath -Raw | ConvertFrom-Json
Write-Output "MSI: $($msiArtifacts[0].FullName)"
Write-Output "MSI SHA-256: $((Get-FileHash -LiteralPath $msiArtifacts[0].FullName -Algorithm SHA256).Hash)"
Write-Output "NSIS: $($nsisArtifacts[0].FullName)"
Write-Output "NSIS SHA-256: $((Get-FileHash -LiteralPath $nsisArtifacts[0].FullName -Algorithm SHA256).Hash)"
Write-Output "Portable executable: $(Join-Path ([IO.Path]::GetFullPath($PortableDestination)) 'jev-switch.exe')"
Write-Output "Portable manifest: $portableManifestPath"
Write-Output "Portable MSI hash: $($portableManifest.installers.($msiArtifacts[0].Name))"
