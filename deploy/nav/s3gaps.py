"""Studio S3-gap gate: background modes, node size bounds, animate default, console parity.

Runs display.py's harness (server, browser, page, report) over s3rows.py's rows.
Usage and exit codes are display.py's. The rAF counter is installed here, before any row runs.

Ponytail: rAF is counted by wrapping window.requestAnimationFrame; a caller that captured the
original before the wrapper went in is not counted, so 0 is a lower bound on a parked page.
"""
import sys

import display
import displaylib
import s3rows

displaylib.INSTALL = displaylib.INSTALL.replace(
    "return true;\n})()",
    "const raf = window.requestAnimationFrame.bind(window);\n"
    "  window.requestAnimationFrame = (cb) => { window.__raf = (window.__raf || 0) + 1; return raf(cb); };\n"
    "  return true;\n})()",
)
display.ROWS = (s3rows.row_background, s3rows.row_size_bounds, s3rows.row_animate_default, s3rows.row_console)

if __name__ == "__main__":
    sys.exit(display.main())
