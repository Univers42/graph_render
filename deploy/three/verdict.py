"""The row verdict, from the nav gate's own module: one implementation, not a second copy.

A row is a row whatever gate asked for it, and the shape of one — what it claimed, what it
measured, and why a row that could not run is NOT-RUN rather than PASS — belongs in one place
(`deploy/nav/verdict.py`). This module loads that file by path, because importing it by name
from a directory that holds a `verdict.py` of its own would find this file instead.
"""

import importlib.util
from pathlib import Path

_SOURCE = Path(__file__).resolve().parent.parent / "nav" / "verdict.py"
_spec = importlib.util.spec_from_file_location("nav_verdict", _SOURCE)
if _spec is None or _spec.loader is None:  # pragma: no cover - the file is in the repository
    raise ImportError(f"the shared row verdict is missing: {_SOURCE}")
_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_module)

row = _module.row

__all__ = ["row"]
