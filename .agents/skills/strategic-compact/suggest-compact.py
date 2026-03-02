#!/usr/bin/env python3
"""Suggest strategic context compaction at logical boundaries.

Works in two modes:
1) Manual checkpoint mode (`--tool Edit` / `--tool Write`)
2) Hook payload mode (`--from-hook`, reads JSON from stdin)
"""

import argparse
import json
import os
from datetime import datetime, timezone
from pathlib import Path

STATE_FILE = Path.home() / ".codex" / "homunculus" / "compact-state.json"
DEFAULT_THRESHOLD = 50
DEFAULT_INTERVAL = 25


def _load_state() -> dict:
    try:
        return json.loads(STATE_FILE.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return {
            "tool_calls": 0,
            "last_suggested_at": 0,
            "updated_at": "",
        }


def _save_state(state: dict) -> None:
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    tmp = STATE_FILE.parent / f".{STATE_FILE.name}.tmp.{os.getpid()}"
    tmp.write_text(json.dumps(state, indent=2))
    os.replace(tmp, STATE_FILE)


def _tool_from_hook_stdin() -> str:
    raw = input_data = ""
    try:
        raw = __import__("sys").stdin.read()
        if not raw.strip():
            return ""
        input_data = json.loads(raw)
    except Exception:
        return ""

    return str(input_data.get("tool_name") or input_data.get("tool") or "").strip()


def main() -> int:
    parser = argparse.ArgumentParser(description="Strategic compact suggestion helper")
    parser.add_argument("--tool", default="", help="Tool name for this checkpoint (e.g., Edit, Write)")
    parser.add_argument("--from-hook", action="store_true", help="Read tool info from stdin JSON payload")
    parser.add_argument("--threshold", type=int, default=DEFAULT_THRESHOLD, help="First suggestion threshold")
    parser.add_argument("--interval", type=int, default=DEFAULT_INTERVAL, help="Suggestion repeat interval")
    parser.add_argument("--reset", action="store_true", help="Reset compact counters")
    args = parser.parse_args()

    state = _load_state()

    if args.reset:
        state = {
            "tool_calls": 0,
            "last_suggested_at": 0,
            "updated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        }
        _save_state(state)
        print("Strategic compact state reset.")
        return 0

    tool = args.tool.strip()
    if args.from_hook:
        hooked_tool = _tool_from_hook_stdin()
        if hooked_tool:
            tool = hooked_tool

    if not tool:
        tool = "checkpoint"

    state["tool_calls"] = int(state.get("tool_calls", 0)) + 1
    state["updated_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

    calls = state["tool_calls"]
    threshold = max(1, args.threshold)
    interval = max(1, args.interval)

    should_suggest = False
    if calls >= threshold:
        last = int(state.get("last_suggested_at", 0))
        if last == 0 or calls - last >= interval:
            should_suggest = True

    if should_suggest:
        state["last_suggested_at"] = calls
        print(
            "STRATEGIC COMPACT SUGGESTION: "
            f"{calls} tracked tool checkpoints reached after '{tool}'. "
            "If you just crossed a phase boundary, run `/compact` with a short summary "
            "(e.g. '/compact Next phase: implement auth middleware')."
        )

    _save_state(state)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
