param(
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [string]$Alias = 'jev-switch-release'
)

$ErrorActionPreference = 'Stop'
$outputRoot = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$keystore = Join-Path $outputRoot 'jev-switch-release.jks'
$base64 = Join-Path $outputRoot 'jev-switch-release.jks.base64'
if (Test-Path -LiteralPath $keystore) {
    throw "Refusing to overwrite an existing keystore: $keystore"
}

$storeSecure = Read-Host 'Keystore password' -AsSecureString
$keySecure = Read-Host 'Key password' -AsSecureString
function Convert-SecureToPlain([Security.SecureString]$Value) {
    $ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($Value)
    try { return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
    finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
}
$storePassword = Convert-SecureToPlain $storeSecure
$keyPassword = Convert-SecureToPlain $keySecure

# Use JKS so the keystore and private-key passwords remain independent. Android's
# apksigner accepts this format directly; PKCS12 silently discards -keypass.
& keytool -genkeypair -v -keystore $keystore -storetype JKS -storepass $storePassword -keypass $keyPassword -alias $Alias -keyalg RSA -keysize 4096 -validity 10000 -dname 'CN=Jev-Switch, OU=Release, O=Jev-Switch, L=Unknown, ST=Unknown, C=US'
if ($LASTEXITCODE -ne 0) { throw 'keytool failed to create the Jev-Switch keystore.' }

$bytes = [IO.File]::ReadAllBytes($keystore)
[Convert]::ToBase64String($bytes) | Set-Content -LiteralPath $base64 -NoNewline
Write-Host "Created dedicated keystore: $keystore"
Write-Host "Created base64 payload: $base64"
Write-Host 'Keep both files offline. Do not commit them or paste their contents into chat.'
