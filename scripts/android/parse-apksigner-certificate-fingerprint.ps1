param(
    [Parameter(Mandatory = $true)]
    [string[]]$VerificationOutput
)

$ErrorActionPreference = 'Stop'
$digestPattern = '(?i)SHA-256[^\r\n]*?(?<digest>(?:[0-9a-f]{2}[:\s-]?){31}[0-9a-f]{2})'

$verificationText = $VerificationOutput -join "`n"
$match = [regex]::Match($verificationText, $digestPattern)
if ($match.Success) {
    Write-Output ($match.Groups['digest'].Value -replace '[:\s-]', '').ToUpperInvariant()
    return
}

throw 'Could not extract the Android signer SHA-256 certificate fingerprint.'
