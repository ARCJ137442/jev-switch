param(
    [Parameter(Mandatory = $true)]
    [string]$ApiKeyPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$daemonPath = Join-Path $repoRoot 'rs/target/release/jev-switch.exe'
$configSource = Join-Path $repoRoot 'rs/providers.example.toml'
foreach ($required in @($daemonPath, $configSource, (Join-Path $repoRoot 'ui/dist/index.html'))) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required TypeSafe test input is missing: $required"
    }
}

if (-not (Test-Path -LiteralPath $ApiKeyPath -PathType Leaf)) {
    throw 'The TypeSafe API key file was not found.'
}
$apiKey = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $ApiKeyPath).Path, [Text.Encoding]::UTF8).Trim()
if ($apiKey.StartsWith('{')) {
    $keyDocument = ConvertFrom-Json -InputObject $apiKey
    $apiKey = @($keyDocument.api_key, $keyDocument.key, $keyDocument.token) | Where-Object { $_ } | Select-Object -First 1
}
if ([string]::IsNullOrWhiteSpace($apiKey) -or $apiKey.Contains("`n") -or $apiKey.Contains("`r")) {
    throw 'The TypeSafe API key file must contain one plaintext key or a JSON api_key/key/token field.'
}

$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()

$targetRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot 'src-tauri/target'))
$testRoot = Join-Path $targetRoot "typesafe-official-test-$([guid]::NewGuid().ToString('N'))"
$configDir = Join-Path $testRoot 'config'
$dataDir = Join-Path $testRoot 'data'
$configPath = Join-Path $configDir 'providers.toml'
$stdoutPath = Join-Path $testRoot 'daemon.stdout.log'
$stderrPath = Join-Path $testRoot 'daemon.stderr.log'
$daemon = $null
$processEnvironment = @('TYPESAFE_API_KEY', 'JEV_SWITCH_CONFIG', 'JEV_SWITCH_DATA_DIR', 'JEV_SWITCH_MODE', 'JEV_BIND', 'JEV_UI_DIST')
$oldEnvironment = @{}
foreach ($name in $processEnvironment) {
    $oldEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

try {
    New-Item -ItemType Directory -Path $configDir, $dataDir | Out-Null
    $config = [IO.File]::ReadAllText($configSource)
    $pattern = '(?ms)(^\[providers\.typesafe\]\s*.*?^enabled\s*=\s*)false'
    $config = [regex]::Replace($config, $pattern, '${1}true', 1)
    if (-not $config.Contains("[providers.typesafe]")) {
        throw 'The example config does not contain the TypeSafe provider block.'
    }
    [IO.File]::WriteAllText($configPath, $config, [Text.UTF8Encoding]::new($false))

    [Environment]::SetEnvironmentVariable('TYPESAFE_API_KEY', $apiKey, 'Process')
    [Environment]::SetEnvironmentVariable('JEV_SWITCH_CONFIG', $configPath, 'Process')
    [Environment]::SetEnvironmentVariable('JEV_SWITCH_DATA_DIR', $dataDir, 'Process')
    [Environment]::SetEnvironmentVariable('JEV_SWITCH_MODE', 'local', 'Process')
    [Environment]::SetEnvironmentVariable('JEV_BIND', "127.0.0.1:$port", 'Process')
    [Environment]::SetEnvironmentVariable('JEV_UI_DIST', (Join-Path $repoRoot 'ui/dist'), 'Process')
    $daemon = Start-Process -FilePath $daemonPath -WorkingDirectory $repoRoot -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath

    $base = "http://127.0.0.1:$port"
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $healthy = $false
    while ([DateTime]::UtcNow -lt $deadline -and -not $daemon.HasExited) {
        try {
            $health = Invoke-RestMethod -Uri "$base/health" -TimeoutSec 2
            if ($health.status -eq 'ok' -and $health.product -eq 'jev-switch') {
                $healthy = $true
                break
            }
        } catch { Start-Sleep -Milliseconds 200 }
    }
    if (-not $healthy) { throw 'The isolated Jev-Switch daemon did not become healthy.' }

    $request = @{
        model = 'jev-latest'
        state = 'The deployment is ready for launch.'
        questions = [ordered]@{
            readiness = @{
                type = 'noul'
                instructions = 'Does the state say the deployment is ready?'
                criteria = @{ true = 'It explicitly says the deployment is ready.'; false = 'It does not say the deployment is ready.' }
            }
            status = @{
                type = 'choice'
                instructions = 'Choose the status that best matches the state.'
                criteria = @{ ready = 'The deployment is ready.'; blocked = 'The deployment is blocked.' }
            }
            score = @{
                type = 'score'
                instructions = 'Rate the deployment readiness.'
                criteria = @('Not ready', 'Ready')
            }
        }
    } | ConvertTo-Json -Depth 12 -Compress

    $started = [Diagnostics.Stopwatch]::StartNew()
    $response = Invoke-WebRequest -Method Post -Uri "$base/v1/admin/providers/typesafe/invoke" `
        -ContentType 'application/json' -Body $request -TimeoutSec 120
    $started.Stop()
    $body = ConvertFrom-Json -InputObject $response.Content
    $expected = @{ readiness = 'noul'; status = 'choice'; score = 'score' }
    foreach ($questionId in $expected.Keys) {
        if ($body.answers.$questionId.type -ne $expected[$questionId]) {
            throw "Official TypeSafe response did not preserve the expected '$questionId' answer type."
        }
    }
    if ($null -eq $body.usage) { throw 'Official TypeSafe response did not include usage.' }

    [pscustomobject]@{
        provider_kind = 'typesafe'
        endpoint = 'https://api.typesafe.ai/v1/systemone'
        upstream_model = $body.model
        status = [int]$response.StatusCode
        request_id = $response.Headers['x-jev-request-id']
        answer_types = ($body.answers.PSObject.Properties | ForEach-Object { "$($_.Name):$($_.Value.type)" }) -join ', '
        score_legend_validated = ($body.answers.score.legend -is [System.Management.Automation.PSCustomObject])
        input_tokens = $body.usage.input_tokens
        output_tokens = $body.usage.output_tokens
        latency_ms = $started.ElapsedMilliseconds
    } | Format-List
}
finally {
    [Environment]::SetEnvironmentVariable('TYPESAFE_API_KEY', $null, 'Process')
    foreach ($name in $processEnvironment | Where-Object { $_ -ne 'TYPESAFE_API_KEY' }) {
        [Environment]::SetEnvironmentVariable($name, $oldEnvironment[$name], 'Process')
    }
    $apiKey = $null

    if ($daemon -and -not $daemon.HasExited) {
        Stop-Process -Id $daemon.Id -Force -ErrorAction SilentlyContinue
        $daemon.WaitForExit(5000) | Out-Null
    }
    if (Test-Path -LiteralPath $testRoot) {
        $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
        if (-not $resolvedTestRoot.StartsWith($targetRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to remove a test directory outside src-tauri/target.'
        }
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
    }
}
