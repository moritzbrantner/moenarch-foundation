#!/usr/bin/env python3
from pathlib import Path
import runpy
import sys

REPO_SCRIPT = Path(__file__).resolve().parents[1] / "skills" / "moenarch-agent-loop" / "scripts" / "agent_loop_queue.py"
INSTALLED_SCRIPT = Path.home() / ".codex" / "skills" / "moenarch-agent-loop" / "scripts" / "agent_loop_queue.py"
SCRIPT = REPO_SCRIPT if REPO_SCRIPT.exists() else INSTALLED_SCRIPT

if not SCRIPT.exists():
    raise SystemExit(f"Missing agent-loop helper script: {SCRIPT}")

sys.path.insert(0, str(SCRIPT.parent))
sys.argv[0] = str(SCRIPT)
runpy.run_path(str(SCRIPT), run_name="__main__")
