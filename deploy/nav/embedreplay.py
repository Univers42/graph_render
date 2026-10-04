"""The embed gate's replay step: `app/embed.html`'s Replay button, streaming a JSONL file of
ingest-v1 batches into `<graph-studio>.applyDeltas` a batch at a time.

The page does the work and says what it did on `window.__embed.replay` (`app/src/embedReplay.ts`);
this module drives it and reads three rows back. Every count the rows judge is one a host could
read — `applied` per batch, the refused line's `name`, and the element's own
`view.frame().nodeCount` — so a row passes only for what the host API says happened.

Reused rather than rewritten: `deltarows.wait_grown`'s poll (`deltarows.py:91`) and its
`POLL_S`, and `embedpage`'s element path and heard-list. NOT reused: `deltarows.drawn` and
`wait_grown` read `liverows.HOST`, which is `document.querySelector('graph-studio')` and is null
on this page — the element sits in `#frame`'s own shadow root, which is why `embedpage.ELEMENT`
exists. `drawn` below is that reader on the element this page really has.

Caveat: the refusal's name is the SDK's typed-error subclass, `BuildRefusedError`
(`crates/graph-sdk-js/src/extend.ts:31`), because the batch goes in through the build staging
path; `IngestInvalid` is the wire *code* under it (code 4, `crates/graph-wasm/src/errors.rs`),
and `crates/graph-wasm/src/service.rs:60` is where a repeated node id becomes that code.
`docs/contract/delta.md:141` names the code, so a host that keys on the name keys on
`BuildRefusedError`. Measured and pinned: see `REFUSAL_NAME`.
"""
import json
import time
from pathlib import Path

import embedpage
import verdict
from deltarows import POLL_S

# The file the button streams, and the button itself, as the served tree and the page name them.
REPLAY = "fixtures/embed/replay.jsonl"
BUTTON = "#replay"

# What `fixtures/embed/replay.jsonl` holds, as its rows promise: six lines, five of them eight
# nodes, and the sixth — `REFUSAL_LINE`, which is also the last — one node line 1 already added.
NODE_BATCH = 8
REFUSAL_LINE = 6
# The refusal's `name`, measured through the host API on 2026-10-04 and cited in the module
# docstring. Ponytail: a pinned string, so a motor that changed the refusal reads as a red row
# rather than as a pass with a different word in it. Direction: an older pack whose SDK predates
# `BuildRefusedError`; the way out is a row per name, or the wire code once the host API carries
# it (it carries `name` and `message` only, `element.ts:140`).
REFUSAL_NAME = "BuildRefusedError"

# The one page state that means the element has no verb to call (`app/src/embedReplay.ts`). Its
# absence is NOT-RUN, never a silent pass — `deltarows.py:28` is the precedent.
NO_VERB = "failed applyDeltas is not on the element"
NOT_RUN_WHY = "`el.applyDeltas` is not on the element, so no batch of the replay was applied"

# How long the drawing has to reach the replay's size. Longer than `deltarows.GROW_S` (2 s)
# because this page has just reloaded and re-laid out 60 nodes first. Ponytail: a fixed cap, not
# a condition. Failing input: a machine slower than the cap at laying out 100 nodes. Direction: it
# reads as a short drawing, so `embed-replay-drawn` fails on a count it never gave up on. Escape
# hatch: a larger GROWN_CAP_S here.
GROWN_CAP_S = 4.0
# How long the replay itself has to reach a settled state, six batches and a refusal.
# Ponytail: same shape as the cap above, and the same escape hatch.
DONE_CAP_S = 60.0

# The page's own account, and the harness's heard events: the latter, not the former, for the
# `graph-error` (the argument `embedrows.step_composed` makes), so a page that recorded its own
# note about an event it never emitted cannot pass this row.
REPLAY_ON_PAGE = "window.__embed === undefined ? null : window.__embed.replay"
HEARD_ON_PAGE = "window.__harness === undefined ? [] : window.__harness.heard"


def drawn(page):
    """How many node rows the view draws now: `deltarows.drawn` on this page's element.

    `deltarows.py:85` reads `liverows.HOST`, which is null here — the element is in `#frame`'s
    shadow root — so this is the same read on `embedpage.ELEMENT`.
    """
    return page.evaluate(f"{embedpage.ELEMENT}.view.frame().nodeCount")


def wait_grown(page, want, seconds=GROWN_CAP_S):
    """Poll the drawn count until it reaches `want`: the count, and how long that took.

    `deltarows.wait_grown` (`deltarows.py:91`) on the element this page has: the same poll, the
    same interval, and the same `watch_workers` inside the loop, because an event sits in the
    socket until something reads it and this client only reads inside `call`.
    """
    began = time.monotonic()
    count = drawn(page)
    while count < want and time.monotonic() - began < seconds:
        page.watch_workers()
        time.sleep(POLL_S)
        count = drawn(page)
    return count, time.monotonic() - began


def button_centre(page):
    """The centre of the page's Replay button in page coordinates, or None when it is not there.

    The shape `gradientrows.click_control` uses (`gradientrows.py:84`), on the page's own light
    DOM: the button is the host page's, not inside the element's shadow root.
    """
    return page.evaluate(f"""
    (() => {{
      const el = document.querySelector('{BUTTON}');
      if (el === null || el.disabled) return null;
      const r = el.getBoundingClientRect();
      return [r.left + r.width / 2, r.top + r.height / 2];
    }})()
    """)


def wait_settled(page, cap=DONE_CAP_S):
    """The replay's own state once it stops running, and how long that took. Never raises.

    `idle` and `running` mean it has not finished; anything else — `done` or `failed <message>`
    — is the page's answer, and the rows judge it. The cap's way out is a bigger `DONE_CAP_S`.
    """
    began = time.monotonic()
    shown = None
    while True:
        page.watch_workers()
        shown = page.evaluate(REPLAY_ON_PAGE)
        state = (shown or {}).get("state")
        if state is not None and state not in ("idle", "running"):
            return shown, time.monotonic() - began
        if time.monotonic() - began > cap:
            return shown, time.monotonic() - began
        time.sleep(POLL_S)


def replay_now(page, hand):
    """Click Replay with a real CDP mouse event and wait for the page's own answer."""
    before = page.evaluate(embedpage.HEARD_COUNT)
    at = button_centre(page)
    if at is not None:
        hand.click(at)
    shown, took = wait_settled(page)
    return {"at": at, "before": before, "shown": shown, "waited": took}


def missing(replayed):
    return (replayed or {}).get("state") == NO_VERB


def not_run(name, expectation):
    """A row that drives the verb cannot be measured without it; that is not a pass."""
    return verdict.row(name, expectation, "no batch of the replay was applied", False, NOT_RUN_WHY)


def row_applied(replayed, batch):
    """Every answering batch, in line order, each answering the batch's own node count."""
    expectation = (f"{REFUSAL_LINE - 1} batches of `{REPLAY}` applied one at a time, each "
                   f"answering `applied` {batch}, and the replay reaching `done`")
    if missing(replayed):
        return not_run("embed-replay-applied", expectation)
    state = (replayed or {}).get("state")
    applied = (replayed or {}).get("applied") or []
    refused = (replayed or {}).get("refused") or []
    measured = (f"state {json.dumps(state)}; applied {json.dumps(applied)}; "
                f"refused {json.dumps(refused)}")
    passed = state == "done" and applied == [batch] * (REFUSAL_LINE - 1)
    return verdict.row("embed-replay-applied", expectation, measured, passed)


def row_refused(names, heard):
    """Exactly one refusal, and one `graph-error` naming the same error.

    Which line refused is not recorded by the page — it keeps names, not line numbers — so the
    row proves it by elimination: `embed-replay-applied` shows the other five lines went in, and
    the replay holds six lines, so the single refused answer is line 6, the one that names a node
    line 1 added. The `graph-error` is read off the harness's own record, not the page's, for the
    reason `embedrows.step_composed` gives.
    """
    expectation = (f"exactly one refusal, line {REFUSAL_LINE} — the batch naming a node an earlier "
                   f"line added — refused as the motor's own typed error `{REFUSAL_NAME}`, and one "
                   f"`graph-error` heard carrying that same name")
    told = [each["detail"].get("error") for each in heard
            if each["type"] == "graph-error" and isinstance(each["detail"], dict)]
    counted = {name: told.count(name) for name in sorted({one for one in told if one is not None})}
    measured = (f"the page refused {json.dumps(names)}; {len(told)} graph-error(s) heard: "
                f"{json.dumps(counted)}")
    return verdict.row("embed-replay-refused", expectation, measured,
                       names == [REFUSAL_NAME] and counted == {REFUSAL_NAME: 1})


def row_drawn(replayed, driven, want):
    """The drawing reaches the size the applied batches promised, and stops there."""
    expectation = (f"the drawing settles at exactly {want} nodes — the fixture's "
                   f"{want - (REFUSAL_LINE - 1) * NODE_BATCH} plus {REFUSAL_LINE - 1} × {NODE_BATCH} "
                   f"— within {GROWN_CAP_S:.0f}s of the last batch's answer")
    if missing(replayed):
        return not_run("embed-replay-drawn", expectation)
    count, took = driven
    measured = (f"{count} nodes drawn of {want} wanted, after {took:.2f}s; state "
                f"{json.dumps((replayed or {}).get('state'))}")
    return verdict.row("embed-replay-drawn", expectation, measured, count == want)


def step_replay(page, ctx):
    """The Replay button, clicked, and the three rows its own account answers to."""
    driven = replay_now(page, ctx["hand"])
    replayed = driven["shown"]
    heard = page.evaluate(f"{HEARD_ON_PAGE}.slice({int(driven['before'])})")
    names = (replayed or {}).get("refused") or []
    want = ctx["fixture"]["nodes"] + (REFUSAL_LINE - 1) * NODE_BATCH
    rows = [row_applied(replayed, NODE_BATCH), row_refused(names, heard)]
    return [*rows, row_drawn(replayed, wait_grown(page, want), want)]


# --------------------------------------------------------------- the fault in the bytes served

# Which line `break-replay`'s served copy is short by. Line 3, so the batches that do arrive are
# lines 1, 2, 4 and 5 and the refusal is still line 6: the drawn count falls and the applied count
# with it, which is the whole point of the control.
DROP_LINE = 3


def drop_line(directory):
    """`replay.jsonl` in `directory` with its third line gone, in place.

    A fault in the bytes the server hands out, like `broken_wasm`'s module: nothing in `app/` or
    `packages/` carries a switch for it. The page fetches four batches and says so, and
    `embed-replay-applied` and `embed-replay-drawn` both read short.
    """
    path = Path(directory) / REPLAY
    lines = path.read_text().splitlines()
    del lines[DROP_LINE - 1]
    path.write_text("\n".join(lines) + "\n")


def served_faults(spec):
    """The faults this run wants in the text the server serves, each a rewrite of one file.

    `embed.served_copy` calls this over a scratch copy of the dist it is about to serve, so a
    fault never touches the build. The wasm fault is `embed.py`'s own; this is the replay's.
    """
    return (drop_line,) if getattr(spec, "replay_missing", False) else ()