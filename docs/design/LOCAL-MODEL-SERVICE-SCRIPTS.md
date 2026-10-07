# 本地模型服务脚本规范

Jev Switch 不为 Laya、StartLux、OneJev 等模型写专用进程逻辑。模型差异全部放在外部服务脚本中，提供商生命周期配置只登记三个结构化动作：`start`、`stop`、`status`。

本文只使用公开的占位符和通用脚本接口，不记录任何机器路径、仓库目录、用户名、模型缓存位置或密钥。具体服务脚本由使用者自行保存到本机私有目录。

脚本必须满足：

- `start` 可以启动独立服务并在服务就绪后退出；不能把模型进程绑定到 Jev Switch 的窗口。
- `stop` 只能根据脚本自己保存的 PID/进程身份停止本次启动的进程，不能按端口或进程名误杀。
- `status` 使用退出码表达状态：`0` = 已就绪，`3` = 已停止，`4` = 启动中，其他 = 未知/失败。
- `start`、`stop` 和 `status` 都要输出脱敏的人类可读摘要；模型 API key 不进入命令参数、日志或 PID 文件。
- 启动前检查模型文件、运行时、端口和资源预算；失败要快速退出并保留日志路径。
- 端口已被未知进程占用时必须拒绝接管。

## 示例生命周期配置（Windows）

```json
{
  "controllable": true,
  "process_policy": "persistent",
  "mode": "manual",
  "program": "<POWERSHELL_EXE>",
  "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "<SCRIPT_ROOT>\\startlux-service.ps1", "-Action", "start"],
  "stop_program": "<POWERSHELL_EXE>",
  "stop_args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "<SCRIPT_ROOT>\\startlux-service.ps1", "-Action", "stop"],
  "status_program": "<POWERSHELL_EXE>",
  "status_args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "<SCRIPT_ROOT>\\startlux-service.ps1", "-Action", "status"]
}
```

`<POWERSHELL_EXE>` 和 `<SCRIPT_ROOT>` 是公开模板占位符，不能原样保存。用户应在自己的配置界面中替换为本机实际路径；公开配置、提交记录和截图应继续保留占位符。

模型运行时路径由脚本自己读取本机私有配置，Jev Switch 不提供、填充或解释这些变量。脚本可以读取环境变量，也可以共同导入一个未提交的 `local-service.config.ps1`/`.env` 文件；Jev Switch 只执行 UI 中登记的脚本路径和参数：

```powershell
$env:LOCAL_SERVICE_SCRIPT_ROOT = '<SCRIPT_ROOT>'
$env:LOCAL_LAYA_PYTHON = '<PYTHON_EXE>'
$env:LOCAL_LAYA_SERVER_SCRIPT = '<LAYA_SERVER_SCRIPT>'
$env:LOCAL_LAYA_SNAPSHOT = '<LAYA_SNAPSHOT_ROOT>'
$env:LOCAL_LAYA_HF_HOME = '<HF_CACHE_ROOT>'
$env:LOCAL_STARTLUX_MODEL_PATH = '<STARTLUX_MODEL_FILE>'
$env:LOCAL_STARTLUX_LLAMASERVER = '<LLAMA_SERVER_EXE>'
$env:LOCAL_STARTLUX_CUDA_RUNTIME = '<CUDA_RUNTIME_ROOT>'
$env:LOCAL_STARTLUX_DECISION_CONFIG = '<STARTLUX_DECISION_CONFIG>'
$env:LOCAL_PYTHON = '<PYTHON_EXE>'
$env:LOCAL_ONEJEV_MODEL_PATH = '<ONEJEV_MODEL_FILE>'
$env:LOCAL_ONEJEV_PROJECTOR_PATH = '<ONEJEV_PROJECTOR_FILE>'
$env:LOCAL_ONEJEV_QEV_PROGRAM = '<QEV_EXE>'
$env:LOCAL_ONEJEV_LLAMASERVER = '<LLAMA_SERVER_EXE>'
```

这些变量只存在于执行本机脚本的进程环境中，不应写进 Git、公开配置导出或截图。

如果多个脚本需要共享同一组路径，推荐由脚本目录中的私有配置文件统一导入，而不是让 Jev Switch 认识这些变量：

```powershell
# local-service.config.ps1（仅保存在本机，并加入 .gitignore）
$env:LOCAL_LAYA_PYTHON = '<PYTHON_EXE>'
$env:LOCAL_LAYA_SNAPSHOT = '<LAYA_SNAPSHOT_ROOT>'
$env:LOCAL_STARTLUX_MODEL_PATH = '<STARTLUX_MODEL_FILE>'
$env:LOCAL_ONEJEV_QEV_PROGRAM = '<QEV_EXE>'

# 每个服务脚本开头自行导入
. (Join-Path $PSScriptRoot 'local-service.config.ps1')
```

Linux/macOS 可以使用同样的 `local-service.config` shell 文件并通过 `. ./local-service.config` 导入；Jev Switch 仍只看到脚本的 `start/stop/status` 接口。

### 环境变量说明

把环境变量理解成“给脚本看的便签”：脚本启动时读取便签内容，知道程序和模型文件在哪里。变量名可以照抄，尖括号里的值必须换成你自己电脑上的真实位置。

| 变量 | 用来告诉脚本什么 | 是否启动对应服务必需 | 值应该填什么 | 是否敏感 |
|---|---|---:|---|---:|
| `LOCAL_SERVICE_SCRIPT_ROOT` | 你的服务脚本所在文件夹 | 仅运行公开验收命令时需要 | 保存 Laya、StartLux、OneJev 脚本的文件夹 | 否，但不要公开个人目录 |
| `LOCAL_LAYA_PYTHON` | Laya 使用哪个 Python | Laya 启动必需 | 已安装 Laya 依赖的 Python 可执行文件 | 否 |
| `LOCAL_LAYA_SERVER_SCRIPT` | Laya 的服务入口脚本 | Laya 启动必需 | 负责启动 Laya HTTP 服务的 `.py` 文件 | 否 |
| `LOCAL_LAYA_SNAPSHOT` | Laya 模型快照放在哪里 | Laya 启动必需 | 已下载、已验证的模型快照文件夹 | 否，但路径属于本机环境 |
| `LOCAL_LAYA_HF_HOME` | Hugging Face 缓存根目录 | Laya 启动必需 | 模型缓存文件夹；配合 offline 模式避免重复下载 | 否，但路径属于本机环境 |
| `LOCAL_STARTLUX_MODEL_PATH` | StartLux 的 GGUF 权重文件 | StartLux 启动必需 | StartLux GGUF 文件的完整路径 | 否，但不要公开个人目录 |
| `LOCAL_STARTLUX_LLAMASERVER` | StartLux 使用的 `llama-server` | StartLux 启动必需 | 已验证版本的 `llama-server` 可执行文件 | 否 |
| `LOCAL_STARTLUX_CUDA_RUNTIME` | StartLux 使用的 CUDA 运行时文件夹 | 使用 CUDA 时必需 | 与 `llama-server` 匹配的 CUDA runtime 文件夹 | 否 |
| `LOCAL_STARTLUX_DECISION_CONFIG` | StartLux 的决策读取配置 | StartLux bridge 启动必需 | `decision_config.json` 文件的完整路径 | 否 |
| `LOCAL_PYTHON` | bridge 使用哪个 Python | StartLux bridge 启动必需 | 能运行 bridge 的 Python 可执行文件 | 否 |
| `LOCAL_ONEJEV_MODEL_PATH` | OneJev 的 GGUF 权重文件 | OneJev 启动必需 | OneJev GGUF 文件的完整路径 | 否，但不要公开个人目录 |
| `LOCAL_ONEJEV_PROJECTOR_PATH` | OneJev 的视觉 projector 文件 | 图片能力必需 | 与 OneJev 权重匹配的 projector 文件 | 否，但不要公开个人目录 |
| `LOCAL_ONEJEV_QEV_PROGRAM` | OneJev 使用的 qev 启动器 | OneJev 启动必需 | qev 可执行文件；也可以在脚本参数中传入 | 否 |
| `LOCAL_ONEJEV_LLAMASERVER` | qev 启动的 llama.cpp 服务 | OneJev GGUF 启动必需 | 与 GGUF 匹配的 `llama-server` 可执行文件 | 否 |

填值时只需要“文件资源管理器复制路径”：找到文件或文件夹，复制它的路径，替换对应的 `<...>`。不要把 API key、密码或 token 放进这些变量；它们只描述程序和模型文件的位置。

## Laya 示例

Laya 脚本需要完成三件事：检查固定本地快照、启动兼容的 SystemOne 服务、用 `/health` 和 `/v1/systemone` 作为 readiness 事实。脚本不应把模型下载、缓存目录或 Python 环境写死在公开代码中。

```powershell
param([ValidateSet('status','start','stop')][string]$Action)
$hostAddress = '127.0.0.1'
$port = 18765
$python = $env:LOCAL_LAYA_PYTHON
$server = $env:LOCAL_LAYA_SERVER_SCRIPT
$snapshot = $env:LOCAL_LAYA_SNAPSHOT

if ($Action -eq 'status') {
  try { Invoke-RestMethod "http://${hostAddress}:$port/health" -TimeoutSec 3 | ConvertTo-Json -Depth 5; exit 0 }
  catch { Write-Output 'Laya is stopped or not ready.'; exit 3 }
}
if ($Action -eq 'stop') {
  # Only stop a PID recorded by this script after verifying executable and arguments.
  # Do not kill by port or process name alone.
  & '<PRIVATE_STOP_HELPER>'
  exit $LASTEXITCODE
}
if (-not (Test-Path $python) -or -not (Test-Path $server) -or -not (Test-Path $snapshot)) {
  throw 'Configure the private Python, server script, and model snapshot paths first.'
}
& $python $server --host $hostAddress --port $port --device cuda
```

## StartLux 示例

StartLux 通常由 `llama-server` 和一个 SystemOne bridge 两个进程组成。启动器应记录两者 PID，等待 bridge `/health`，停止时只处理自己记录且身份匹配的 PID。

```powershell
param([ValidateSet('status','start','stop')][string]$Action)
$llama = $env:LOCAL_STARTLUX_LLAMASERVER
$model = $env:LOCAL_STARTLUX_MODEL_PATH
$runtime = $env:LOCAL_STARTLUX_CUDA_RUNTIME
$bridge = $env:LOCAL_STARTLUX_BRIDGE_SCRIPT
$python = $env:LOCAL_PYTHON
$config = $env:LOCAL_STARTLUX_DECISION_CONFIG
$llamaPort = 18761
$bridgePort = 18760

if ($Action -eq 'status') {
  try { Invoke-RestMethod "http://127.0.0.1:$bridgePort/health" -TimeoutSec 3 | ConvertTo-Json; exit 0 }
  catch { Write-Output 'StartLux is stopped or not ready.'; exit 3 }
}
if ($Action -eq 'start') {
  # Validate files, RAM, VRAM, ports and ownership before detached launch.
  # Start llama-server, then bridge, then wait for bridge readiness.
  & '<PRIVATE_START_HELPER>'
  exit $LASTEXITCODE
}
if ($Action -eq 'stop') {
  & '<PRIVATE_STOP_HELPER>'
  exit $LASTEXITCODE
}
```

## 验收命令

```powershell
$scriptRoot = $env:LOCAL_SERVICE_SCRIPT_ROOT
if ([string]::IsNullOrWhiteSpace($scriptRoot)) { throw 'Set LOCAL_SERVICE_SCRIPT_ROOT to your private service-script directory.' }
$laya = Join-Path $scriptRoot 'laya-service.ps1'
$startlux = Join-Path $scriptRoot 'startlux-service.ps1'
$onejev = Join-Path $scriptRoot 'onejev-service.ps1'
& $laya -Action status
& $startlux -Action status
& $onejev -Action status
```

## 在 Jev Switch 界面中配置

脚本准备好后，配置工作由人在 Providers 页面完成：

1. 打开“提供商”，创建或编辑一个提供商，填写它的 API 地址和模型 ID。
2. 展开“服务启停”，先打开“可控服务”。“允许路由”只决定是否把请求送给它，两者不要混淆。
3. 将“启动程序”填写为本机的 PowerShell、Python 或其他解释器；将“启动参数”逐行填写为脚本路径和 `-Action start`。
4. 将“停止程序”和“停止参数”配置为同一个脚本的 `-Action stop`。
5. 将“状态检查程序”和“状态检查参数”配置为同一个脚本的 `-Action status`。
6. 选择 `persistent`、`session` 或 `external` 进程策略。模型服务通常选择 `persistent`，避免关闭控制台时误停服务。
7. 点击“保存启停配置”，等待保存成功后再点击“检查状态”。只有状态命令返回 `0` 且 API readiness 成功，界面才显示“运行中”。
8. 点击“启动服务”，观察状态从“启动中”变为“运行中”；完成测试后点击“停止服务”，再检查对应端口已经释放。

自动探测开关默认开启。首页会定期探测提供商 API；打开某个提供商的“服务启停”面板时，会额外定期读取该服务的状态命令。关闭某个提供商的自动探测只影响该提供商，不会删除配置，也不会关闭服务。

## 示例逐行说明

下面的示例刻意只使用环境变量和占位符。Windows 的 PowerShell、Linux/macOS 的 shell 或 Python 启动器都可以遵循同一结构：程序路径、参数数组、状态退出码、readiness 和 PID 所有权彼此分开。

### Laya 示例的关键行

| 示例行/机制 | 为什么需要 | Linux/macOS 对应经验 |
|---|---|---|
| `param(... Action ...)` | 让同一个脚本拥有 `status/start/stop` 三个明确动作，Jev Switch 只需登记三组参数 | 用 `case "$1"` 或 `argparse` 实现同样的三个动作 |
| `hostAddress`、`port` | 固定服务监听位置，状态命令和 provider 地址必须指向同一位置 | 使用 `127.0.0.1` 与端口变量，不要扫描端口猜服务 |
| `Invoke-RestMethod ... /health` | 只做轻量 readiness，不发送模型推理请求；HTTP 200 才表示服务可用 | 使用 `curl --fail`、`wget --spider` 或 Python `urllib` |
| `Test-Path $python/$server/$snapshot` | 在启动前快速发现运行时、脚本或模型快照缺失，避免 UI 长时间卡在启动中 | 使用 `test -x`、`test -f`、`test -d` |
| RAM/VRAM 预算检查 | 本地模型可能耗尽整机资源，启动前拒绝比启动后崩溃更可解释 | 使用 `/proc/meminfo`、`free`、`nvidia-smi` 或平台等价工具 |
| `HF_HUB_OFFLINE=1`、`TRANSFORMERS_OFFLINE=1` | 已有本地快照时禁止隐式联网下载，避免每次启动重复下载 | 以 `export` 设置同名变量；没有缓存时快速失败并提示准备缓存 |
| 启动脚本路径和 `-Action start` | 生命周期 API 传入的是结构化 argv，脚本负责模型运行时细节 | 直接执行脚本或 `python service.py start`，不要拼接 `sh -c` |
| 精确父子进程识别 | 停止时只处理该脚本本次创建的进程，避免误杀其他 Python 服务 | 保存 PID/进程组；使用 `kill -- -PGID` 或 supervisor 的 stop API |
| 退出码 `0/3/4` | 避免依赖中文 stdout；daemon 和 Agent 可稳定判断 ready/stopped/starting | Unix 脚本使用 `exit 0/3/4`，systemd 可映射为健康状态 |

### StartLux 示例的关键行

| 示例行/机制 | 为什么需要 | Linux/macOS 对应经验 |
|---|---|---|
| `llama-server` 与 bridge 两个地址 | llama.cpp 只提供底层推理，bridge 才提供 Jev SystemOne 协议；两者不能混成一个健康检查 | 用两个 systemd user service、supervisord program 或两个受控子进程 |
| `--model`/`-m` 与 `--mmproj` | 明确指定模型/视觉 projector，避免脚本依赖当前目录或自动下载 | 用绝对路径或部署目录变量，不把路径写死在公开脚本 |
| `--host 127.0.0.1` | 默认只允许本机网关访问，减少误暴露；局域网开放应由 Jev Switch 单独控制 | 绑定 loopback；公网部署需显式反向代理和鉴权 |
| `Start-Process -RedirectStandard*` | detached 启动仍保留日志，失败时用户能定位，而不是只看到 HTTP 400 | 使用 `nohup ... >stdout 2>stderr &` 或 systemd journal |
| 先等 llama 端口，再等 bridge `/health` | 端口打开不代表模型加载完成；bridge readiness 才能让 UI 显示“运行中” | 先 `curl` 底层健康，再 `curl` bridge 健康 |
| 记录两个 PID 并校验命令行 | 停止时需要同时释放底层和 bridge，且不能停止别人的同名服务 | 保存 PID 文件、进程组 ID 和启动命令 hash |

不要把 API key 放进启动参数或环境变量示例。若确实需要向受信任脚本注入 key，使用 Jev Switch 的敏感变量确认流程，并保证脚本日志脱敏。

## FAQ 与故障排查

**启动按钮一直是“启动中”怎么办？**

先点“检查状态”。如果启动器已经退出但 provider `/health` 未就绪，界面会显示失败并释放互斥锁。检查脚本日志、模型文件、资源门禁和 readiness 地址；不要连续重复点击。

**停止显示 `command_failed` 或 `exit 1` 怎么办？**

确认停止参数与启动参数使用同一个脚本，且脚本保存并校验自己启动的 PID/进程组。不要使用“按端口杀进程”作为通用停止方案。PowerShell 需要检查父子进程链；Linux/macOS 需要检查 process group 或 systemd/supervisor 所有权。

PowerShell 的 `catch { Write-Output ... }` 默认仍可能以退出码 `0` 结束，导致“服务已经停止”被误判成“运行中”。状态脚本的失败分支必须显式 `exit 3`，成功分支显式 `exit 0`；停止脚本也必须在确认端口释放后才返回 `0`。

**为什么显示“运行中”但演练场请求失败？**

“运行中”必须来自状态命令和 HTTP readiness；如果脚本只返回成功但没有真实 `/health`，就会产生假运行状态。把状态程序改为检查实际 provider 地址，并确认模型 ID、协议路径和路由的 `upstream_model` 一致。

**为什么每次启动又下载模型？**

启动脚本应指向固定快照，并设置对应框架的 offline 变量。缓存缺失时应该快速失败，让用户补齐模型文件，而不是在生命周期请求里隐式联网下载。

**OneJev 找不到 `qev` 怎么办？**

把 qev 可执行文件通过本机私有变量或脚本参数提供，并确认 GGUF 与 projector 属于匹配版本。公开配置只保留 `<QEV_EXE>`、`<ONEJEV_MODEL_FILE>` 等占位符。

**自动探测会不会触发模型推理？**

不会。提供商自动探测只访问健康/探测端点；服务启停面板的状态命令也应只检查状态，不发送真实模型问题。真实推理仍由演练场或调用 API 触发。

**Windows 配置能迁移到 Linux 吗？**

能迁移设计，不直接复制命令字符串：保留三个动作、退出码、readiness、资源门禁和所有权原则，把 `powershell.exe`/`Start-Process` 换成 shell/Python/systemd，把 Windows 路径换成 Linux 路径，把进程停止实现换成 process group。这样 Agent 也能根据同一套 lifecycle 能力发现和配置服务。

API 侧只调用 `POST /v1/admin/providers/{id}/lifecycle/status|start|stop`。Jev Switch 负责统一权限、审计、超时和状态呈现；脚本负责模型运行时的具体知识。`status` 的结果必须先于“运行中”文案，不能以启动器进程存在代替模型 HTTP readiness。
