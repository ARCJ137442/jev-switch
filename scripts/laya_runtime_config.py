"""Portable environment and model snapshot resolution for the local Laya server.

This module intentionally has no Laya, Torch, FastAPI, or Hugging Face imports so
that path selection can be checked on a machine that does not have model
dependencies installed yet.
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Mapping, MutableMapping


DEFAULT_SNAPSHOT_REVISION = "1c5edc17a7acd8701df6fc341c0d179f1c62c982"
MODEL_CACHE_NAME = "models--convaiinnovations--laya"


def _default_hf_home(env: Mapping[str, str], *, windows: bool, home: Path) -> Path:
    configured = env.get("HF_HOME")
    if configured:
        return Path(configured).expanduser()

    # Preserve the existing Windows workstation layout when it is present, but
    # do not force that drive on Linux, WSL, containers, or fresh machines.
    windows_cache = Path("E:/hf-cache")
    if windows and windows_cache.is_dir():
        return windows_cache
    return home / ".cache" / "huggingface"


def resolve_environment(
    env: Mapping[str, str] | None = None,
    *,
    windows: bool | None = None,
    home: Path | None = None,
) -> dict[str, str]:
    """Return the environment needed before importing ``laya``.

    Caller-owned values win. Missing cache variables are derived from the
    selected Hugging Face home. Temporary paths follow TMPDIR/TMP/TEMP and are
    otherwise left for Python and the operating system to choose.
    """
    source = dict(os.environ if env is None else env)
    is_windows = os.name == "nt" if windows is None else windows
    user_home = Path.home() if home is None else home
    hf_home = _default_hf_home(source, windows=is_windows, home=user_home)

    result = dict(source)
    result.setdefault("HF_HOME", str(hf_home))
    result.setdefault("HUGGINGFACE_HUB_CACHE", str(Path(result["HF_HOME"]) / "hub"))
    result.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")

    temporary = next(
        (result.get(name) for name in ("TMPDIR", "TMP", "TEMP") if result.get(name)),
        None,
    )
    if temporary:
        result.setdefault("TMPDIR", temporary)
        result.setdefault("TMP", temporary)
        result.setdefault("TEMP", temporary)
    return result


def apply_environment(
    env: MutableMapping[str, str] | None = None,
    *,
    windows: bool | None = None,
    home: Path | None = None,
) -> dict[str, str]:
    """Apply :func:`resolve_environment` to a process environment."""
    target = os.environ if env is None else env
    resolved = resolve_environment(target, windows=windows, home=home)
    for name, value in resolved.items():
        target.setdefault(name, value)
    return resolved


def resolve_snapshot(env: Mapping[str, str] | None = None) -> Path:
    """Resolve the local Laya snapshot without contacting Hugging Face."""
    source = os.environ if env is None else env
    explicit = source.get("LOCAL_LAYA_SNAPSHOT")
    if explicit:
        return Path(explicit).expanduser()
    cache = source.get("HUGGINGFACE_HUB_CACHE")
    if not cache:
        cache = str(_default_hf_home(source, windows=os.name == "nt", home=Path.home()) / "hub")
    return Path(cache).expanduser() / MODEL_CACHE_NAME / "snapshots" / DEFAULT_SNAPSHOT_REVISION


if __name__ == "__main__":
    resolved = apply_environment()
    print(f"HF_HOME={resolved['HF_HOME']}")
    print(f"HUGGINGFACE_HUB_CACHE={resolved['HUGGINGFACE_HUB_CACHE']}")
    print(f"JEV_LAYA_SNAPSHOT={resolve_snapshot(resolved)}")
