param(
    [Parameter(Mandatory = $true)]
    [string]$Base64Payload,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($Base64Payload)) {
    throw 'Windows certificate Base64 payload is empty.'
}

try {
    $bytes = [Convert]::FromBase64String(($Base64Payload -replace '\s', ''))
}
catch {
    throw "Windows certificate Base64 payload is invalid: $($_.Exception.Message)"
}
if ($bytes.Length -lt 256) {
    throw 'Windows certificate payload is too small to be a PFX certificate.'
}

$fullPath = [IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Path (Split-Path -Parent $fullPath) -Force | Out-Null
[IO.File]::WriteAllBytes($fullPath, $bytes)
Write-Output $fullPath
