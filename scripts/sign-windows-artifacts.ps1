param(
    [Parameter(Mandatory = $true)]
    [string[]]$Path,
    [Parameter(Mandatory = $true)]
    [string]$CertificatePath,
    [Parameter(Mandatory = $true)]
    [string]$CertificatePassword,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [string]$MetadataPath
)

$ErrorActionPreference = 'Stop'

function Resolve-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $kitsRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (Test-Path -LiteralPath $kitsRoot) {
        $candidate = Get-ChildItem -LiteralPath $kitsRoot -Filter 'signtool.exe' -Recurse -File |
            Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }

    throw 'signtool.exe was not found. Install the Windows SDK or add its x64 bin directory to PATH.'
}

if (-not (Test-Path -LiteralPath $CertificatePath -PathType Leaf)) {
    throw "Windows signing certificate is missing: $CertificatePath"
}

$signTool = Resolve-SignTool
$files = @(
    $Path |
        ForEach-Object {
            if (-not (Test-Path -LiteralPath $_ -PathType Leaf)) {
                throw "Windows signing input is missing: $_"
            }
            (Resolve-Path -LiteralPath $_).Path
        }
)
if ($files.Count -eq 0) {
    throw 'At least one Windows artifact must be provided for signing.'
}

$records = @()
foreach ($file in $files) {
    $extension = [IO.Path]::GetExtension($file).ToLowerInvariant()
    if ($extension -notin @('.exe', '.msi', '.dll')) {
        throw "Unsupported Authenticode artifact type: $file"
    }

    Write-Host "Signing $file"
    & $signTool sign /fd SHA256 /f $CertificatePath /p $CertificatePassword /tr $TimestampUrl /td SHA256 /a $file
    if ($LASTEXITCODE -ne 0) {
        throw "signtool sign failed for $file with exit code $LASTEXITCODE."
    }

    & $signTool verify /pa /all /v $file
    if ($LASTEXITCODE -ne 0) {
        throw "signtool verification failed for $file with exit code $LASTEXITCODE."
    }

    $signature = Get-AuthenticodeSignature -LiteralPath $file
    $artifactName = [IO.Path]::GetRelativePath((Get-Location).Path, $file).Replace('\', '/')
    $records += [ordered]@{
        artifact = $artifactName
        sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        status = [string]$signature.Status
        signer = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
        thumbprint = if ($signature.SignerCertificate) { $signature.SignerCertificate.Thumbprint } else { $null }
        timestamp = if ($signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Subject } else { $null }
    }
}

if ($MetadataPath) {
    $parent = Split-Path -Parent ([IO.Path]::GetFullPath($MetadataPath))
    if ($parent) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    $metadata = [ordered]@{
        signing = 'Authenticode SHA-256 with RFC 3161 timestamp'
        timestamp_url = $TimestampUrl
        artifacts = $records
    }
    if (Test-Path -LiteralPath $MetadataPath -PathType Leaf) {
        $previous = Get-Content -LiteralPath $MetadataPath -Raw | ConvertFrom-Json
        $metadata.artifacts = @($previous.artifacts) + @($records)
    }
    $metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $MetadataPath -Encoding UTF8
}

Write-Host "Signed and verified $($files.Count) Windows artifact(s) with $signTool."
