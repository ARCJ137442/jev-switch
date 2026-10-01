param(
    [Parameter(Mandatory = $true)]
    [string]$KeystorePath,
    [Parameter(Mandatory = $true)]
    [string]$Alias
)

$ErrorActionPreference = 'Stop'
$keystore = (Resolve-Path -LiteralPath $KeystorePath).Path
$storePassword = [Environment]::GetEnvironmentVariable('ANDROID_KEYSTORE_PASSWORD')
if ([string]::IsNullOrWhiteSpace($storePassword)) {
    throw 'ANDROID_KEYSTORE_PASSWORD is required to inspect the release keystore.'
}

$prefix = [IO.File]::ReadAllBytes($keystore) | Select-Object -First 4
if ($prefix.Count -lt 4) {
    throw 'The Android release keystore is truncated.'
}

$keystoreType = if ($prefix[0] -eq 0xFE -and $prefix[1] -eq 0xED -and $prefix[2] -eq 0xFE -and $prefix[3] -eq 0xED) {
    'JKS'
} elseif ($prefix[0] -eq 0x30) {
    'PKCS12'
} else {
    throw 'The Android release keystore has an unrecognized binary format.'
}

& keytool -list -storetype $keystoreType -keystore $keystore -storepass $storePassword -alias $Alias 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "The Android release keystore or alias cannot be opened as $keystoreType with the configured store password."
}

Write-Output $keystoreType
