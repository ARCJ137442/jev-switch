$ErrorActionPreference = 'Stop'
$parser = Join-Path $PSScriptRoot 'parse-apksigner-certificate-fingerprint.ps1'
$expected = '0123456789ABCDEF' * 4
$contiguous = 'Signer #1 certificate SHA-256 digest: ' + $expected.ToLowerInvariant()
$octets = for ($i = 0; $i -lt $expected.Length; $i += 2) { $expected.Substring($i, 2) }
$colonSeparated = 'Signer #1 certificate SHA-256 digest: ' + ($octets -join ':')
$spaceSeparated = 'Signer #1 certificate SHA-256 digest: ' + ($octets -join ' ')

foreach ($fixture in @($contiguous, $colonSeparated, $spaceSeparated)) {
    $actual = & $parser -VerificationOutput @('Verified using v2 scheme (APK Signature Scheme v2): true', $fixture)
    if ($actual -ne $expected) {
        throw "Unexpected normalized fingerprint: $actual"
    }
}

try {
    & $parser -VerificationOutput @('Signer #1 certificate SHA-256 digest: not-a-fingerprint') | Out-Null
    throw 'Malformed fingerprint unexpectedly passed parsing.'
}
catch {
    if ($_.Exception.Message -eq 'Malformed fingerprint unexpectedly passed parsing.') { throw }
}

Write-Host 'Contiguous, colon-separated and space-separated APK signer fingerprints passed.'
