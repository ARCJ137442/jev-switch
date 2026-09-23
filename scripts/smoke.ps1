# smoke.ps1 — Jev-Switch 端到端冒烟（11 步 / 9 断言；硬断言失败 exit 1）
#
# 用法（仓库根执行）：
#   powershell -File scripts/smoke.ps1     # Windows PowerShell 5.1（本机主用）
#   pwsh     -File scripts/smoke.ps1       # PowerShell 7+（同脚本兼容）
#   bash scripts/smoke.sh                  # Linux CI / Git Bash 对照版（同一断言集）
#
# 断言集（顺序执行、逐项 ✓/✗；步骤 2–8 为硬断言，9/10 为可选步骤）：
#   1  前置：cargo build + 复制 providers.example.toml 到临时配置（$env:JEV_SWITCH_CONFIG）
#           + 后台起 jev-switch.exe + 轮询 /health ≤30s（含 11435 端口预占检查）
#   2  GET  /health                → JSON、status=="ok"、有 version
#   3  GET  /v1/models             → object=="list"；data 为数组且每项 upstream 非空
#                                    （不可路由过滤生效）；含 upstreams
#   4  POST /v1/systemone 缺 criteria → 400 + 错误体含 error
#   5  POST /v1/systemone 未知 model → 404 + upstream:null
#   6  GET  /v1/admin/providers    → 含 api_key_masked；全文无 "api_key":
#   7  PUT  /v1/admin/routes 环（a→b、b→a）→ 400 + 文案含 环|cycle；
#                                    仓库内 providers.example.toml SHA256 未变
#   8  POST /v1/admin/providers/nope/probe → 404
#   9  （可选）GET 到的每个 provider 逐个 probe：HTTP 契约 200 为硬；
#                                    全部上游不可达仅 warning 不失败
#  10  （可选）laya-english 在 data 且 127.0.0.1:18765 可达 → 真实 noul POST
#                                    期望 200 且有 answers；否则 SKIP 警告继续
#  11  收尾：杀进程、删临时配置；汇总 PASS x/y, SKIP z（任一硬 ✗ → exit 1）
#
# 已知坑（任务书 / 06 §四风险表，务必保留）：
#   - PS5.1 Invoke-WebRequest 异常的响应体在 $_.ErrorDetails.Message
#     （GetResponseStream 已被预读，读了是空）
#   - 必须用临时配置副本：PUT /v1/admin/routes 会写 toml —— 防污染仓库
#     providers.example.toml（步骤 7 另有 hash 复核）
#   - 环境通常无 Vercel key、Laya 服务未启动 → 步骤 9/10 可选，不通仅
#     warning / SKIP（06 计划 §四风险表）
#   - 密钥不进日志：本脚本不读写任何真实 key（测试假值也不需要 —— 示例
#     配置无明文 api_key）

$ErrorActionPreference = 'Stop'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}

$repoRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exampleCfg = Join-Path $repoRoot 'rs\providers.example.toml'
$daemonExe  = Join-Path $repoRoot 'rs\target\debug\jev-switch.exe'
$base       = 'http://127.0.0.1:11435'
$assertTotal = 9   # 步骤 2–10

$script:pass = 0
$script:fail = 0
$script:skip = 0
$script:modelIds = @()

function Write-Step([int]$n, [string]$msg) { Write-Host ("[{0}/11] {1}" -f $n, $msg) }
function Ok   ([string]$m) { $script:pass++; Write-Host ("       OK   {0}" -f $m) }
function Bad  ([string]$m) { $script:fail++; Write-Host ("       FAIL {0}" -f $m) }
function Skipped([string]$m) { $script:skip++; Write-Host ("       SKIP {0}" -f $m) }
function Warn ([string]$m) { Write-Host ("       warn {0}" -f $m) }

# 单步包装：步骤内异常 = 该步 FAIL（不中断后续步骤，便于一次看全）
function Invoke-Step([int]$n, [string]$title, [scriptblock]$body) {
    Write-Step $n $title
    try { & $body } catch { Bad ("unexpected: {0}" -f $_) }
}

# HTTP 请求：正常返回 @{Status,Body}；4xx/5xx 不抛（状态码 + 错误体一起返回）。
# PS5.1 坑：异常响应体在 $_.ErrorDetails.Message。
function Invoke-Http {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Uri,
        [string]$Body = $null
    )
    try {
        $p = @{
            Uri             = $Uri
            Method          = $Method
            UseBasicParsing = $true
            TimeoutSec      = 10
            ErrorAction     = 'Stop'
        }
        # PS5.1 坑：[string]$Body 把 $null 强转成 ""，`$null -ne $Body` 恒真 →
        # GET 也带 Body → “Cannot send a content-body with this verb-type”。
        # 仅当真正有内容时才附 Body + ContentType。
        if (-not [string]::IsNullOrEmpty($Body)) {
            $p['Body'] = $Body
            $p['ContentType'] = 'application/json'
        }
        $r = Invoke-WebRequest @p
        return @{ Status = [int]$r.StatusCode; Body = [string]$r.Content }
    } catch {
        $status = 0
        if ($_.Exception.Response) {
            try { $status = [int]$_.Exception.Response.StatusCode } catch { $status = 0 }
        }
        $body = ''
        if ($_.ErrorDetails -and $_.ErrorDetails.Message) { $body = $_.ErrorDetails.Message }
        elseif ($_.Exception.Message) { $body = $_.Exception.Message }
        return @{ Status = $status; Body = $body }
    }
}

function ConvertTo-JObject([string]$s) {
    try { return ($s | ConvertFrom-Json) } catch { return $null }
}

$hashBefore = $null
$daemon = $null
$tmpDir = $null
$exitCode = 1

try {
    # ══ 步骤 1 · 前置：hash → build → 临时配置 → 起服务 → 轮询 ══════════
    Write-Step 1 '前置：编译 + 临时配置 + 后台起 daemon + /health 轮询 ≤30s'

    if (-not (Test-Path $exampleCfg)) { throw "missing $exampleCfg" }
    $hashBefore = (Get-FileHash -Path $exampleCfg -Algorithm SHA256).Hash

    # 端口预占检查：已有实例在听 → 直接失败（防断言打到旧进程/旧配置）
    $portBusy = $false
    try {
        $probe = Invoke-WebRequest -Uri "$base/health" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        $portBusy = $true
        $null = $probe
    } catch {
        $portBusy = ($null -ne $_.Exception.Response)  # 有 HTTP 响应 = 占用；连接拒绝 = 空闲
    }
    if ($portBusy) { throw "port 11435 already in use — 请先停掉既有 jev-switch 再跑 smoke" }

    # 编译（default-members = daemon bin）
    $manifest = Join-Path $repoRoot 'rs\Cargo.toml'
    Write-Host ("       cargo build --manifest-path {0}" -f $manifest)
    & cargo build --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
    if (-not (Test-Path $daemonExe)) { throw "daemon binary not found: $daemonExe" }

    # 临时配置副本（PUT 写 toml 只污染这里）
    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("jev-smoke-{0}" -f $PID)
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
    $tempCfg = Join-Path $tmpDir 'providers.toml'
    Copy-Item -Path $exampleCfg -Destination $tempCfg -Force
    $env:JEV_SWITCH_CONFIG = $tempCfg
    Write-Host ("       JEV_SWITCH_CONFIG={0}" -f $tempCfg)

    # 后台起服务（继承当前 env；stdout/err 落临时目录便于诊断）
    $daemon = Start-Process -FilePath $daemonExe `
        -WorkingDirectory $repoRoot `
        -RedirectStandardOutput (Join-Path $tmpDir 'daemon.out.log') `
        -RedirectStandardError  (Join-Path $tmpDir 'daemon.err.log') `
        -PassThru -NoNewWindow

    $pollStart = Get-Date
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $up = $false
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($daemon.HasExited) { break }
        $code = 0
        try {
            $r = Invoke-WebRequest -Uri "$base/health" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
            $code = [int]$r.StatusCode
        } catch { $code = 0 }
        if ($code -eq 200) { $up = $true; break }
        Start-Sleep -Milliseconds 500
    }
    if (-not $up) {
        $errLog = Join-Path $tmpDir 'daemon.err.log'
        if (Test-Path $errLog) {
            Write-Host '       daemon.err.log tail:'
            Get-Content $errLog -Tail 20 | ForEach-Object { Write-Host ("         {0}" -f $_) }
        }
        if ($daemon.HasExited) { throw "daemon exited early (code $($daemon.ExitCode))" }
        throw "daemon /health not ready within 30s"
    }
    $elapsed = [Math]::Round(((Get-Date) - $pollStart).TotalSeconds, 1)
    Ok ("daemon up in ~{0}s (temp config, port 11435)" -f $elapsed)

    # ══ 步骤 2 · GET /health ═══════════════════════════════════════════
    Invoke-Step 2 'GET /health → JSON status=ok + version' {
        $r = Invoke-Http -Method GET -Uri "$base/health"
        $j = ConvertTo-JObject $r.Body
        if ($r.Status -eq 200 -and $j -and $j.status -eq 'ok' -and -not [string]::IsNullOrEmpty([string]$j.version)) {
            Ok ("status=ok version={0}" -f $j.version)
        } else {
            Bad ("status={0} body={1}" -f $r.Status, $r.Body)
        }
    }

    # ══ 步骤 3 · GET /v1/models 形状 + 不可路由过滤 ═══════════════════
    Invoke-Step 3 'GET /v1/models → object=list；data 每项 upstream 非空；有 upstreams' {
        $r = Invoke-Http -Method GET -Uri "$base/v1/models"
        $j = ConvertTo-JObject $r.Body
        # 形状用原始文本断言（PS5.1 ConvertFrom-Json 单元素数组会解包，别用它判类型）
        $shapeOk = ($r.Body -match '"object"\s*:\s*"list"') -and
                   ($r.Body -match '"data"\s*:\s*\[') -and
                   ($r.Body -match '"upstreams"\s*:\s*\[')
        $data = @()
        if ($j -and $null -ne $j.data) { $data = @($j.data | Where-Object { $null -ne $_ }) }
        $allUpstream = $true
        foreach ($m in $data) {
            if ([string]::IsNullOrEmpty([string]$m.upstream)) { $allUpstream = $false }
        }
        $script:modelIds = @($data | ForEach-Object { [string]$_.id })
        if ($r.Status -eq 200 -and $shapeOk -and $data.Count -ge 1 -and $allUpstream) {
            Ok ("object=list data=[{0}] upstreams OK" -f ($script:modelIds -join ', '))
        } else {
            Bad ("status={0} shapeOk={1} count={2} allUpstream={3} body={4}" -f
                $r.Status, $shapeOk, $data.Count, $allUpstream, $r.Body)
        }
    }

    # ══ 步骤 4 · 缺 criteria → 400 ════════════════════════════════════
    Invoke-Step 4 'POST /v1/systemone 缺 criteria → 400 + error' {
        $body = '{"model":"jev","state":"smoke","questions":{"q":{"type":"noul","instructions":"x"}}}'
        $r = Invoke-Http -Method POST -Uri "$base/v1/systemone" -Body $body
        $j = ConvertTo-JObject $r.Body
        $hasErr = $j -and ($j.PSObject.Properties.Name -contains 'error')
        if ($r.Status -eq 400 -and $hasErr) {
            Ok ("400 error={0}" -f ([string]$j.error))
        } else {
            Bad ("status={0} hasError={1} body={2}" -f $r.Status, $hasErr, $r.Body)
        }
    }

    # ══ 步骤 5 · 未知 model → 404 + upstream:null ═════════════════════
    Invoke-Step 5 'POST /v1/systemone 未知 model → 404 + upstream:null' {
        $body = '{"model":"definitely-not-a-model","state":"smoke","questions":{"q":{"type":"noul","instructions":"x","criteria":{"true":"y","false":"n"}}}}'
        $r = Invoke-Http -Method POST -Uri "$base/v1/systemone" -Body $body
        $upstreamNull = $r.Body -match '"upstream"\s*:\s*null'
        if ($r.Status -eq 404 -and $upstreamNull) {
            Ok '404 upstream=null'
        } else {
            Bad ("status={0} upstreamNull={1} body={2}" -f $r.Status, $upstreamNull, $r.Body)
        }
    }

    # ══ 步骤 6 · GET providers 掩码（全文无 "api_key":）════════════════
    Invoke-Step 6 'GET /v1/admin/providers → api_key_masked；全文无 "api_key":' {
        $r = Invoke-Http -Method GET -Uri "$base/v1/admin/providers"
        $hasMasked = $r.Body -match '"api_key_masked"'
        $hasPlain  = $r.Body -match '"api_key"\s*:'   # 裸 api_key 字段（不含 masked/set）
        if ($r.Status -eq 200 -and $hasMasked -and -not $hasPlain) {
            Ok '200 + api_key_masked present; no "api_key": field'
        } else {
            Bad ("status={0} masked={1} plain={2} body={3}" -f $r.Status, $hasMasked, $hasPlain, $r.Body)
        }
    }

    # ══ 步骤 7 · PUT 环 → 400 + 仓库示例 toml hash 未变 ═══════════════
    Invoke-Step 7 'PUT /v1/admin/routes 环 payload → 400 + 环/cycle；example.toml hash 不变' {
        $body = '{"routes":[{"left":"a","right":"b","priority":1},{"left":"b","right":"a","priority":2}]}'
        $r = Invoke-Http -Method PUT -Uri "$base/v1/admin/routes" -Body $body
        # 响应体中文按 UTF-8 解码可能错位 —— ASCII 子串 "cycle" 兜底（文案两者都有）
        $msgOk = ($r.Body -match '环') -or ($r.Body -match 'cycle')
        $hashAfter = (Get-FileHash -Path $exampleCfg -Algorithm SHA256).Hash
        $hashOk = ($hashBefore -eq $hashAfter)
        if ($r.Status -eq 400 -and $msgOk -and $hashOk) {
            Ok ("400 cycle-rejected; example.toml hash unchanged ({0})" -f $hashBefore.Substring(0, 12))
        } else {
            Bad ("status={0} msgOk={1} hashOk={2} body={3}" -f $r.Status, $msgOk, $hashOk, $r.Body)
        }
    }

    # ══ 步骤 8 · probe 未知 id → 404 ══════════════════════════════════
    Invoke-Step 8 'POST /v1/admin/providers/nope/probe → 404' {
        $r = Invoke-Http -Method POST -Uri "$base/v1/admin/providers/nope/probe" -Body '{}'
        if ($r.Status -eq 404) {
            Ok '404'
        } else {
            Bad ("status={0} body={1}" -f $r.Status, $r.Body)
        }
    }

    # ══ 步骤 9 ·（可选）逐 provider probe ══════════════════════════════
    Invoke-Step 9 '（可选）每个 provider probe：HTTP 200 为硬；全不可达仅 warning' {
        $r = Invoke-Http -Method GET -Uri "$base/v1/admin/providers"
        $j = ConvertTo-JObject $r.Body
        $ids = @()
        if ($j -and $j.providers) { $ids = @($j.providers | ForEach-Object { [string]$_.id }) }
        if ($ids.Count -eq 0) {
            Skipped 'no providers listed'
            return
        }
        $httpOk = $true
        $reachable = 0
        foreach ($id in $ids) {
            $p = Invoke-Http -Method POST -Uri ("{0}/v1/admin/providers/{1}/probe" -f $base, $id) -Body '{}'
            if ($p.Status -ne 200) {
                $httpOk = $false
                Warn ("probe {0} HTTP {1} body={2}" -f $id, $p.Status, $p.Body)
                continue
            }
            $pj = ConvertTo-JObject $p.Body
            if ($pj -and $pj.ok -eq $true) {
                $reachable++
            } else {
                # 不可达（通常本机无 Vercel key / Laya 未启动）→ 仅 warning
                Warn ("probe {0} unreachable (ok=false, status={1})" -f $id, $(if ($pj) { $pj.status } else { '?' }))
            }
        }
        if (-not $httpOk) {
            Bad 'probe endpoint non-200（端点契约破坏 → 硬失败）'
        } elseif ($reachable -eq 0) {
            Ok ("probe HTTP 200 × {0}（全部上游不可达 — warning 不计失败）" -f $ids.Count)
        } else {
            Ok ("probe reachable {0}/{1}" -f $reachable, $ids.Count)
        }
    }

    # ══ 步骤 10 ·（可选）真实推理 POST ═════════════════════════════════
    Invoke-Step 10 '（可选）Laya 可达 → 真实 noul POST 期望 200+answers；否则 SKIP' {
        $inData = $script:modelIds -contains 'laya-english'
        # 探活：拿到任意 HTTP 响应（200–599）= 服务在听；000/连接拒绝 = 不在
        $h = Invoke-Http -Method GET -Uri 'http://127.0.0.1:18765/health'
        $layaUp = ($h.Status -ge 200 -and $h.Status -le 599)
        if (-not $inData -or -not $layaUp) {
            Skipped ("laya-english in data={0}, 127.0.0.1:18765 reachable={1}（真实推理依赖本机环境）" -f $inData, $layaUp)
            return
        }
        $body = '{"model":"laya-english","state":"smoke","questions":{"q":{"type":"noul","instructions":"is this smoke test passing?","criteria":{"true":"yes","false":"no"}}}}'
        $r = Invoke-Http -Method POST -Uri "$base/v1/systemone" -Body $body
        $j = ConvertTo-JObject $r.Body
        $hasAnswers = $j -and ($null -ne $j.answers)
        if ($r.Status -eq 200 -and $hasAnswers) {
            Ok '200 + answers（真实 Laya 推理）'
        } else {
            Bad ("status={0} hasAnswers={1} body={2}" -f $r.Status, $hasAnswers, $r.Body)
        }
    }

    $exitCode = if ($script:fail -gt 0) { 1 } else { 0 }
} catch {
    Write-Host ("       FAIL unexpected: {0}" -f $_) -ForegroundColor Red
    $exitCode = 1
} finally {
    # ══ 步骤 11 · 收尾：杀进程、删临时配置、汇总 ═════════════════════
    Write-Step 11 '收尾：停 daemon、删临时配置、汇总'
    if ($null -ne $daemon) {
        try {
            if (-not $daemon.HasExited) {
                Stop-Process -Id $daemon.Id -Force -ErrorAction SilentlyContinue
                $null = $daemon.WaitForExit(5000)
            }
        } catch {}
    }
    if ($null -ne $tmpDir -and (Test-Path $tmpDir)) {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Write-Host ''
    Write-Host ("== PASS {0}/{1}, SKIP {2} ==" -f $script:pass, $assertTotal, $script:skip)
    if ($script:fail -gt 0) {
        Write-Host ("== {0} hard assertion(s) FAILED → exit 1 ==" -f $script:fail) -ForegroundColor Red
    }
}

exit $exitCode
