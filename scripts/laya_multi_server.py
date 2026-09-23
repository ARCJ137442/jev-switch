"""单地址 · 多模型 Laya Jev 服务（同提供商不同模型）。

设计裁定（用户，2026-09-23）：**一个 API 地址 ↔ 一个提供商**（LM Studio 同款标准）。
Laya 的三个 checkpoint 不是三个提供商，而是**同一个提供商（Laya 本地）下的不同模型**。
本服务在 `127.0.0.1:18765` 一个地址上同时暴露：

  laya-english         英语 checkpoint（convaiinnovations/laya，421M ModernBERT-large）
  laya-multilingual    多语言 checkpoint（laya-multilingual，322M mmBERT，100+ 语言）
  laya-typed-decisions typed-decisions checkpoint（显式调用才加载）
  laya-router          **Router 版**：按文种/脚本自动分派（英文→english，非拉丁→multilingual）

别名兼容：`convaiinnovations/laya`、`convaiinnovations/laya-multilingual`、
`english` / `multilingual` 等 laya 官方别名均可用。

端点（与既有单模型服务及官方 Jev 一致）：
  POST /v1/systemone           —— Jev 标准端点
  POST /api/alpha/decisions    —— OpenRouter 兼容别名
  GET  /v1/models              —— 模型清单（含 router）
  GET  /health                 —— 健康检查 + 已加载 checkpoint

响应在既有形态上增加路由元数据：
  x_backend_model = 实际命中的 checkpoint（english / multilingual / typed-decisions）
  x_routing       = 完整路由决策（model / reason / detection…，Router 版展示分派依据）

启动：
  python scripts/laya_multi_server.py [--host 127.0.0.1] [--port 18765] [--device cuda]
权重：全部走本机 HF 快照路径（零网络）；启动时预载 english + multilingual 双权重。

实现要点：
- 复用 `laya.Router`（0.3.4 内置）：`models` 直接指到本地快照 = 离线；
  `preload(['english','multilingual'])` 双权重常驻（路由微秒级、无冷启动抖动）；
  钉死 id 走 `predict(..., model=key)`（显式优先，绕过脚本检测）；
  `laya-router` 走自动检测（非拉丁文 → multilingual，拉丁英语 → english）。
- 推理经全局锁串行（本地单机演示；避免 Router LRU 与并发推理竞态）。
"""
from __future__ import annotations

import argparse
import logging
import os
import threading
import time
from typing import Any, Dict, Optional

# ---- 环境（必须在 import laya 之前设置，沿用既有服务） -----------------------
os.environ.setdefault("HF_ENDPOINT", "https://hf-mirror.com")
os.environ.setdefault("HF_HOME", "E:/hf-cache")
os.environ.setdefault("HUGGINGFACE_HUB_CACHE", "E:/hf-cache/hub")
os.environ.setdefault("TMP", "E:/tmp")
os.environ.setdefault("TEMP", "E:/tmp")
os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

import laya
from laya import Router

# 本机完整快照（三个 safetensors 齐全；revision 漂移不影响——路径直载零网络）
SNAP = "E:/hf-cache/hub/models--convaiinnovations--laya/snapshots/1c5edc17a7acd8701df6fc341c0d179f1c62c982"

logging.basicConfig(level=logging.INFO,
                    format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger("jev-laya-multi")

# ---- 公开 model id → Router 键 / router 模式 --------------------------------
PIN: Dict[str, str] = {
    "laya-english": "english",
    "laya-multilingual": "multilingual",
    "laya-typed-decisions": "typed-decisions",
    # 官方 HF id 别名
    "convaiinnovations/laya": "english",
    "convaiinnovations/laya-multilingual": "multilingual",
    "convaiinnovations/laya-typed-decisions": "typed-decisions",
    # laya 官方短别名
    "english": "english",
    "multilingual": "multilingual",
    "typed-decisions": "typed-decisions",
}
ROUTER_IDS = {"laya-router", "router", "laya-auto"}

INFER_LOCK = threading.Lock()
_router: Optional[Router] = None


def get_router() -> Router:
    global _router
    if _router is None:
        raise RuntimeError("router not initialised")
    return _router


def init_router(device: str) -> None:
    """构建 Router 并预载双权重（本地快照路径，零网络）。"""
    global _router
    models = {
        "english": (SNAP, None),
        "multilingual": (SNAP, "multilingual"),
        "typed-decisions": (SNAP, "typed-decisions"),
    }
    log.info("initialising Router (local snapshot, preload english+multilingual) ...")
    t0 = time.perf_counter()
    _router = Router(models=models, device=device, max_loaded=2, default="english")
    _router.preload(["english", "multilingual"])
    log.info("Router ready in %.1fs, loaded=%s", time.perf_counter() - t0, _router.loaded)


# ---- Jev schema 映射（与既有单模型服务逐字一致） -----------------------------
def _normalize_question(qid: str, q: Dict[str, Any]) -> Dict[str, Any]:
    t = q.get("type", "choice").lower()
    ins = q.get("instructions") or q.get("question") or ""
    if not ins:
        raise ValueError(f"question {qid!r}: instructions is required")

    if t == "choice":
        crit = q.get("criteria")
        if crit is None and "options" in q:
            opts = q["options"]
            if isinstance(opts, list):
                crit = {str(o): None for o in opts}
            elif isinstance(opts, dict):
                crit = opts
        if not crit:
            raise ValueError(f"question {qid!r}: choice needs criteria or options")
        if isinstance(crit, list):
            crit = {str(c): None for c in crit}
        return {"type": "choice", "instructions": ins, "criteria": crit}

    if t == "score":
        crit = q.get("criteria") or q.get("levels")
        if isinstance(crit, dict):
            crit = list(crit.values())
        if not crit:
            raise ValueError(f"question {qid!r}: score needs criteria or levels")
        return {"type": "score", "instructions": ins, "criteria": crit}

    if t == "noul":
        return {"type": "noul", "instructions": ins}

    raise ValueError(f"question {qid!r}: unknown type {t!r}")


def _normalize_questions(questions: Dict[str, Any]) -> Dict[str, Any]:
    return {qid: _normalize_question(qid, q) for qid, q in questions.items()}


class JevRequest(BaseModel):
    model: str = "laya-router"
    state: Any = Field(...)
    questions: Dict[str, Dict[str, Any]]

    class Config:
        extra = "allow"


def available_ids() -> list:
    return sorted(list(PIN) + sorted(ROUTER_IDS))


def create_app(device: str) -> FastAPI:
    init_router(device)
    app = FastAPI(title="Jev-Compatible HTTP Server (Laya multi-model)",
                  version="0.2.0")

    @app.get("/health")
    def health():
        r = get_router()
        return {"status": "ok", "mode": "multi-model", "device": device,
                "loaded": r.loaded,
                "models": sorted(list(PIN) + sorted(ROUTER_IDS))}

    @app.get("/v1/models")
    def models():
        return {
            "object": "list",
            "data": [
                {"id": "laya-english", "object": "model", "backend": "english"},
                {"id": "laya-multilingual", "object": "model", "backend": "multilingual"},
                {"id": "laya-router", "object": "model", "backend": "auto(script)"},
                {"id": "laya-typed-decisions", "object": "model", "backend": "typed-decisions"},
            ],
        }

    async def _handle(req: JevRequest):
        try:
            questions_norm = _normalize_questions(req.questions)
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))

        mid = (req.model or "").strip()
        if mid in ROUTER_IDS:
            pin = None          # 自动：脚本/语言检测分派
        elif mid in PIN:
            pin = PIN[mid]      # 钉死 checkpoint
        else:
            raise HTTPException(
                status_code=400,
                detail=f"unknown model {req.model!r}; available: {', '.join(available_ids())}")

        t0 = time.perf_counter()
        try:
            with INFER_LOCK:
                if pin is None:
                    result = get_router().predict(req.state, questions_norm)
                else:
                    result = get_router().predict(req.state, questions_norm, model=pin)
        except Exception as e:
            log.exception("inference failed")
            raise HTTPException(status_code=500, detail=f"inference error: {e}")
        latency_ms = (time.perf_counter() - t0) * 1000

        routing = result.pop("routing", None) or {}
        result["model"] = req.model                      # 回显公开 id
        result.setdefault("usage", {})
        result["usage"]["latency_ms"] = round(latency_ms, 2)
        result["x_backend_model"] = routing.get("model")  # 实际命中的 checkpoint
        if pin is None:
            result["x_routing"] = routing                # Router 版附完整决策
        return result

    @app.post("/v1/systemone")
    async def systemone(req: JevRequest):
        return await _handle(req)

    @app.post("/api/alpha/decisions")
    async def alpha_decisions(req: JevRequest):
        return await _handle(req)

    return app


def main():
    ap = argparse.ArgumentParser(description="单地址多模型 Laya Jev 服务")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=18765)
    ap.add_argument("--device", default="cuda", choices=["cuda", "cpu"])
    args = ap.parse_args()

    app = create_app(args.device)
    import uvicorn
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
