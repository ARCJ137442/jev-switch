$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("jev-keystore-type-test-{0}" -f [Guid]::NewGuid().ToString('N'))
$storePassword = 'JevSwitchTestStore123'
$previousStorePassword = $env:ANDROID_KEYSTORE_PASSWORD
New-Item -ItemType Directory -Path $testRoot | Out-Null

function New-TestKeystore([string]$Path, [string]$Type, [string]$KeyPassword) {
    & keytool -genkeypair -noprompt -keystore $Path -storetype $Type `
        -storepass $storePassword -keypass $KeyPassword -alias detection-test `
        -keyalg RSA -keysize 2048 -validity 2 -dname 'CN=Jev-Switch Test' 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Could not generate $Type test keystore." }
}

try {
    $env:ANDROID_KEYSTORE_PASSWORD = $storePassword
    $pkcs12Path = Join-Path $testRoot 'pkcs12.keystore'
    $jksPath = Join-Path $testRoot 'jks.keystore'
    New-TestKeystore $pkcs12Path 'PKCS12' 'IgnoredPkcs12KeyPassword123'
    New-TestKeystore $jksPath 'JKS' 'JksPrivateKey123'

    $pkcs12Type = & (Join-Path $PSScriptRoot 'detect-release-keystore-type.ps1') `
        -KeystorePath $pkcs12Path -Alias 'detection-test'
    if ($pkcs12Type -ne 'PKCS12') { throw "Expected PKCS12 detection; got $pkcs12Type." }

    $jksType = & (Join-Path $PSScriptRoot 'detect-release-keystore-type.ps1') `
        -KeystorePath $jksPath -Alias 'detection-test'
    if ($jksType -ne 'JKS') { throw "Expected JKS detection; got $jksType." }

    Write-Host 'PKCS12 and JKS keystore detection passed.'
}
finally {
    if ($null -eq $previousStorePassword) {
        Remove-Item Env:ANDROID_KEYSTORE_PASSWORD -ErrorAction SilentlyContinue
    }
    else {
        $env:ANDROID_KEYSTORE_PASSWORD = $previousStorePassword
    }
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
