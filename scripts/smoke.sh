#!/usr/bin/env bash
# smoke.sh — Jev-Switch 端到端冒烟（与 scripts/smoke.ps1 同一断言集）
#
# 用法（仓库根执行）：
#   bash scripts/smoke.sh          # Linux CI / Git Bash / macOS
#   powershell -File scripts/smoke.ps1   # Windows 主用版（11 步说明见其文件头）
#
# 断言集（步骤 1–11 与 smoke.ps1 逐项对应；步骤 2–8 硬断言，9/10 可选；
# 任一硬断言 ✗ → exit 1；汇总 PASS x/y, SKIP z）：
#   1  cargo build + 复制 providers.example.toml → JEV_SWITCH_CONFIG（临时）
#      + 后台起 daemon + /health 轮询 ≤30s（含端口预占检查）
#   2  GET  /health               → status=ok + version
#   3  GET  /v1/models            → object=list；data 每项 upstream 非空；有 upstreams
#   4  POST /v1/systemone 缺 criteria → 400 + error
#   5  POST /v1/systemone 未知 model → 404 + upstream:null
#   6  GET  /v1/admin/providers   → api_key_masked；全文无 "api_key":
#   7  PUT  /v1/admin/routes 环   → 400 + 环|cycle；example.toml SHA256 未变
#   8  POST .../providers/nope/probe → 404
#   9  （可选）逐 provider probe：HTTP 200 为硬；全不可达仅 warning
#  10  （可选）Laya 可达 → 真实 noul POST 200+answers；否则 SKIP
#  11  杀进程、删临时配置、汇总
#
# 依赖：bash + curl + jq + sha256sum（Git Bash / GNU coreutils 自带）。
# 坑位同 ps1：必须临时配置副本（PUT 会写 toml）；无 Vercel key / Laya 未启
# 时步骤 9/10 仅 warning / SKIP（06 §四风险表）。

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE="http://127.0.0.1:11435"
EXAMPLE_CFG="$REPO_ROOT/rs/providers.example.toml"
ASSERT_TOTAL=9   # 步骤 2–10

pass=0 fail=0 skip=0
MODEL_IDS=""

step()  { printf '[%s/11] %s\n' "$1" "$2"; }
ok()    { pass=$((pass + 1)); printf '       OK   %s\n' "$1"; }
bad()   { fail=$((fail + 1)); printf '       FAIL %s\n' "$1"; }
skipped(){ skip=$((skip + 1)); printf '       SKIP %s\n' "$1"; }
warn()  { printf '       warn %s\n' "$1"; }

die_start() { printf '       FAIL %s\n' "$1"; printf '\n== PASS 0/%d, SKIP 0 ==\n' "$ASSERT_TOTAL"; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die_start "missing dependency: $1"
}

# http METHOD URL [BODY] → HTTP_CODE / HTTP_BODY（非 0 退出不中断脚本）
BODY_FILE=""
http() {
  local method=$1 url=$2 body=${3:-}
  local args=(-sS -m 15 -X "$method" -o "$BODY_FILE" -w '%{http_code}'
              -H 'Content-Type: application/json')
  if [ -n "$body" ]; then args+=(-d "$body"); fi
  HTTP_CODE=$(curl "${args[@]}" "$url" 2>/dev/null) || HTTP_CODE=000
  HTTP_BODY=$(cat "$BODY_FILE" 2>/dev/null || true)
}

# jq 断言：jq -e FILTER <<<body；0 = 通过
jq_ok() { jq -e "$1" >/dev/null 2>&1 <<<"$2"; }

# jq -r 取值：Windows 版 jq.exe 输出 CRLF（实测 jq-1.8.1 PE 控制台程序），
# 不剥 \r 会污染 id（"laya\r" 进 URL → curl rc=3）与字符串比较（"true\r"≠"true"）
jqv() { jq -r "$1" <<<"$2" 2>/dev/null | tr -d '\r'; }

cleanup() {
  if [ -n "${DAEMON_PID:-}" ]; then
    kill "$DAEMON_PID" 2>/dev/null || true
    wait "$DAEMON_PID" 2>/dev/null || true
  fi
  if [ -n "${TMP_DIR:-}" ] && [ -d "${TMP_DIR:-}" ]; then
    rm -rf "$TMP_DIR"
  fi
}

# ══ 步骤 1 · 前置 ═══════════════════════════════════════════════════
step 1 '前置：编译 + 临时配置 + 后台起 daemon + /health 轮询 ≤30s'

require_cmd curl
require_cmd jq
require_cmd cargo
require_cmd sha256sum

[ -f "$EXAMPLE_CFG" ] || die_start "missing $EXAMPLE_CFG"
HASH_BEFORE=$(sha256sum "$EXAMPLE_CFG" | awk '{print $1}')

# 端口预占检查：已有实例在听 → 失败（防断言打到旧进程）
# 注意：失败时 curl 已输出 "000"，再 `|| echo 000` 会拼成 "000000" —— 只许重赋值
busy_code=$(curl -s -m 2 -o /dev/null -w '%{http_code}' "$BASE/health" 2>/dev/null) || busy_code=000
[ "$busy_code" != "000" ] && [ -n "$busy_code" ] && \
  die_start "port 11435 already in use (health=$busy_code) — 请先停掉既有 jev-switch"

printf '       cargo build --manifest-path %s/rs/Cargo.toml\n' "$REPO_ROOT"
if ! cargo build --manifest-path "$REPO_ROOT/rs/Cargo.toml"; then
  die_start "cargo build failed"
fi

BIN="$REPO_ROOT/rs/target/debug/jev-switch"
[ -x "$BIN.exe" ] && BIN="$BIN.exe"
[ -x "$BIN" ] || die_start "daemon binary not found: $BIN"

TMP_DIR=$(mktemp -d)
BODY_FILE="$TMP_DIR/http.body"
DAEMON_PID=""
trap cleanup EXIT

cp "$EXAMPLE_CFG" "$TMP_DIR/providers.toml"
export JEV_SWITCH_CONFIG="$TMP_DIR/providers.toml"
printf '       JEV_SWITCH_CONFIG=%s\n' "$JEV_SWITCH_CONFIG"

"$BIN" >"$TMP_DIR/daemon.out.log" 2>"$TMP_DIR/daemon.err.log" &
DAEMON_PID=$!

up=0
i=0
while [ "$i" -lt 60 ]; do
  if ! kill -0 "$DAEMON_PID" 2>/dev/null; then break; fi
  code=$(curl -s -m 2 -o /dev/null -w '%{http_code}' "$BASE/health" 2>/dev/null) || code=000
  if [ "$code" = "200" ]; then up=1; break; fi
  sleep 0.5
  i=$((i + 1))
done
if [ "$up" != "1" ]; then
  [ -f "$TMP_DIR/daemon.err.log" ] && {
    printf '       daemon.err.log tail:\n'
    tail -n 20 "$TMP_DIR/daemon.err.log" | sed 's/^/         /'
  }
  if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    die_start "daemon exited early"
  fi
  die_start "daemon /health not ready within 30s"
fi
ok "daemon up (temp config, port 11435)"

# ══ 步骤 2 · GET /health ════════════════════════════════════════════
step 2 'GET /health → JSON status=ok + version'
http GET "$BASE/health"
if [ "$HTTP_CODE" = "200" ] && jq_ok '.status=="ok" and (.version|type=="string" and length>0)' "$HTTP_BODY"; then
  ok "status=ok version=$(jqv '.version' "$HTTP_BODY")"
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 3 · GET /v1/models ═════════════════════════════════════════
step 3 'GET /v1/models → object=list；data 每项 upstream 非空；有 upstreams'
http GET "$BASE/v1/models"
models_filter='
  .object=="list"
  and (.data|type=="array")
  and (.upstreams|type=="array")
  and ((.data|length) >= 1)
  and (all(.data[]; (.upstream|type)=="string" and (.upstream|length)>0))
'
if [ "$HTTP_CODE" = "200" ] && jq_ok "$models_filter" "$HTTP_BODY"; then
  MODEL_IDS=$(jq -r '.data[].id' <<<"$HTTP_BODY" 2>/dev/null | tr -d '\r' || true)
  ok "object=list data=[$(printf '%s' "$MODEL_IDS" | tr '\n' ',' | sed 's/,$//')] upstreams OK"
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 4 · 缺 criteria → 400 ══════════════════════════════════════
step 4 'POST /v1/systemone 缺 criteria → 400 + error'
NO_CRITERIA='{"model":"jev","state":"smoke","questions":{"q":{"type":"noul","instructions":"x"}}}'
http POST "$BASE/v1/systemone" "$NO_CRITERIA"
if [ "$HTTP_CODE" = "400" ] && jq_ok 'has("error")' "$HTTP_BODY"; then
  ok "400 error=$(jqv '.error' "$HTTP_BODY")"
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 5 · 未知 model → 404 ═══════════════════════════════════════
step 5 'POST /v1/systemone 未知 model → 404 + upstream:null'
UNKNOWN='{"model":"definitely-not-a-model","state":"smoke","questions":{"q":{"type":"noul","instructions":"x","criteria":{"true":"y","false":"n"}}}}'
http POST "$BASE/v1/systemone" "$UNKNOWN"
if [ "$HTTP_CODE" = "404" ] && jq_ok 'has("upstream") and .upstream==null' "$HTTP_BODY"; then
  ok '404 upstream=null'
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 6 · GET providers 掩码 ═════════════════════════════════════
step 6 'GET /v1/admin/providers → api_key_masked；全文无 "api_key":'
http GET "$BASE/v1/admin/providers"
providers_filter='
  (.providers|type=="array")
  and (all(.providers[]; has("api_key_masked")))
  and ([.providers[] | has("api_key")] | any | not)
'
if [ "$HTTP_CODE" = "200" ] && jq_ok "$providers_filter" "$HTTP_BODY" \
   && ! grep -q '"api_key":' "$BODY_FILE"; then
  ok '200 + api_key_masked present; no "api_key": field'
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 7 · PUT 环 → 400 + hash 不变 ═══════════════════════════════
step 7 'PUT /v1/admin/routes 环 payload → 400 + 环/cycle；example.toml hash 不变'
CYCLE='{"routes":[{"left":"a","right":"b","priority":1},{"left":"b","right":"a","priority":2}]}'
http PUT "$BASE/v1/admin/routes" "$CYCLE"
HASH_AFTER=$(sha256sum "$EXAMPLE_CFG" | awk '{print $1}')
msg_ok=0
grep -qE '环|cycle' "$BODY_FILE" && msg_ok=1
hash_ok=0
[ "$HASH_BEFORE" = "$HASH_AFTER" ] && hash_ok=1
if [ "$HTTP_CODE" = "400" ] && [ "$msg_ok" = "1" ] && [ "$hash_ok" = "1" ]; then
  ok "400 cycle-rejected; example.toml hash unchanged (${HASH_BEFORE:0:12})"
else
  bad "status=$HTTP_CODE msgOk=$msg_ok hashOk=$hash_ok body=$HTTP_BODY"
fi

# ══ 步骤 8 · probe 未知 id → 404 ════════════════════════════════════
step 8 'POST /v1/admin/providers/nope/probe → 404'
http POST "$BASE/v1/admin/providers/nope/probe" '{}'
if [ "$HTTP_CODE" = "404" ]; then
  ok '404'
else
  bad "status=$HTTP_CODE body=$HTTP_BODY"
fi

# ══ 步骤 9 ·（可选）逐 provider probe ═══════════════════════════════
step 9 '（可选）每个 provider probe：HTTP 200 为硬；全不可达仅 warning'
http GET "$BASE/v1/admin/providers"
IDS=$(jq -r '.providers[].id' <<<"$HTTP_BODY" 2>/dev/null | tr -d '\r' || true)
if [ -z "$IDS" ]; then
  skipped 'no providers listed'
else
  http_ok=1 reachable=0 n=0
  for id in $IDS; do
    n=$((n + 1))
    http POST "$BASE/v1/admin/providers/$id/probe" '{}'
    if [ "$HTTP_CODE" != "200" ]; then
      http_ok=0
      warn "probe $id HTTP $HTTP_CODE body=$HTTP_BODY"
      continue
    fi
    if [ "$(jqv '.ok' "$HTTP_BODY")" = "true" ]; then
      reachable=$((reachable + 1))
    else
      warn "probe $id unreachable (ok=false, status=$(jqv '.status' "$HTTP_BODY"))"
    fi
  done
  if [ "$http_ok" != "1" ]; then
    bad 'probe endpoint non-200（端点契约破坏 → 硬失败）'
  elif [ "$reachable" -eq 0 ]; then
    ok "probe HTTP 200 × $n（全部上游不可达 — warning 不计失败）"
  else
    ok "probe reachable $reachable/$n"
  fi
fi

# ══ 步骤 10 ·（可选）真实推理 POST ══════════════════════════════════
step 10 '（可选）Laya 可达 → 真实 noul POST 期望 200+answers；否则 SKIP'
in_data=0
for mid in $MODEL_IDS; do
  [ "$mid" = "laya-english" ] && in_data=1
done
laya_code=$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:18765/health 2>/dev/null) || laya_code=000
laya_up=0
case "$laya_code" in
  000|'') laya_up=0 ;;
  *) laya_up=1 ;;
esac
if [ "$in_data" != "1" ] || [ "$laya_up" != "1" ]; then
  skipped "laya-english in data=$([ "$in_data" = 1 ] && echo true || echo false), 127.0.0.1:18765 reachable=$([ "$laya_up" = 1 ] && echo true || echo false)（真实推理依赖本机环境）"
else
  REAL='{"model":"laya-english","state":"smoke","questions":{"q":{"type":"noul","instructions":"is this smoke test passing?","criteria":{"true":"yes","false":"no"}}}}'
  http POST "$BASE/v1/systemone" "$REAL"
  if [ "$HTTP_CODE" = "200" ] && jq_ok '.answers|type=="object"' "$HTTP_BODY"; then
    ok '200 + answers（真实 Laya 推理）'
  else
    bad "status=$HTTP_CODE body=$HTTP_BODY"
  fi
fi

# ══ 步骤 11 · 收尾（trap cleanup 杀进程 + 删临时目录）═══════════════
step 11 '收尾：停 daemon、删临时配置、汇总'
cleanup
trap - EXIT
DAEMON_PID=""
TMP_DIR=""

printf '\n== PASS %d/%d, SKIP %d ==\n' "$pass" "$ASSERT_TOTAL" "$skip"
if [ "$fail" -gt 0 ]; then
  printf '== %d hard assertion(s) FAILED → exit 1 ==\n' "$fail"
  exit 1
fi
exit 0
