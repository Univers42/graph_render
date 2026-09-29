"""The page: how the probe reads the studio state and drives its registry."""
import json
import time
from frbase import BUSY_S, Missing, SETTLE_S, Unjudgeable

# ------------------------------------------------------------------------------- the page

STATE = """
(() => {
  const host = document.querySelector('graph-studio');
  if (host === null || host.studio === null || host.view === null) return null;
  const at = host.studio.store.get();
  const view = host.view;
  const arr = (value) => (value === undefined || value === null) ? null : Array.from(value);
  const pick = (source, names) => {
    for (const name of names) {
      if (source !== undefined && source !== null && source[name] !== undefined && source[name] !== null) {
        return arr(source[name]);
      }
    }
    return null;
  };
  const style = typeof view.style === "function" ? view.style() : null;
  const frame = typeof view.frame === "function" ? view.frame() : null;
  const meta = at.meta === undefined ? null : at.meta;
  const failed = at.error === undefined ? null : at.error;
  return {
    busy: at.busy.length,
    layoutCalls: at.layoutCalls === undefined ? null : at.layoutCalls,
    filter: at.settings.filter,
    groups: at.settings.groups === undefined ? null : at.settings.groups,
    colourBy: at.settings.appearance.colourBy,
    analysisId: at.settings.analysis,
    values: at.analysis === undefined ? null : arr(at.analysis === null ? null : at.analysis.values),
    analyses: at.catalog === null ? null : Array.from(at.catalog.analyses),
    error: failed === null ? null : { title: failed.title, detail: failed.detail },
    meta: meta === null ? null : {
      nodeCount: meta.nodeCount,
      ids: arr(meta.ids), labels: arr(meta.labels), kinds: arr(meta.kinds),
      tags: arr(meta.tags), tag: arr(meta.tag),
      dbs: arr(meta.dbs), db: arr(meta.db),
      paths: arr(meta.paths), path: arr(meta.path),
      group: arr(meta.group), degree: arr(meta.degree), maxDegree: meta.maxDegree,
    },
    palette: style === null ? null : arr(style.palette),
    colours: style === null ? null : arr(style.colours),
    hidden: style === null ? null : arr(style.hidden),
    search: pick(style, ["search", "highlight", "matches"]),
    frame: frame === null ? null : { x: arr(frame.x), y: arr(frame.y), nodeCount: frame.nodeCount },
    camera: view.camera(),
    actions: host.studio.registry.actions.map((action) => ({
      id: action.id, alias: action.alias, params: action.params.map((param) => param.name),
    })),
  };
})()
"""

DISPATCH = """
(async () => {
  const host = document.querySelector('graph-studio');
  if (host === null || host.studio === null) {
    return { ok: false, error: { title: 'NoStudio', detail: 'the page has no studio on it' } };
  }
  const action = host.studio.registry.find(%action%);
  if (action === undefined) {
    return { ok: false, error: { title: 'NoSuchAction', detail: 'the registry has no action %action%' } };
  }
  const args = {};
  for (const pair of Object.entries(%args%)) {
    // The row names the argument the brief gives it; the control may spell that parameter
    // differently (`colour` takes `name` here and `by` in the shipped action), so the
    // parameter is taken from the action the registry holds rather than failed over a word.
    const spec = action.params.find((param) => param.name === pair[0])
      ?? (action.params.length === 1 ? action.params[0] : undefined);
    if (spec === undefined) {
      return { ok: false, error: { title: 'UnknownParam', detail: action.alias + ' takes no ' + pair[0] } };
    }
    args[spec.name] = pair[1];
  }
  try {
    const entry = await host.studio.dispatch(action.id, args);
    return {
      ok: entry.ok === true, message: entry.message,
      error: entry.error === null ? null : { title: entry.error.title, detail: entry.error.detail },
    };
  } catch (failure) {
    return { ok: false, error: { title: 'Threw', detail: String(failure && failure.message || failure) } };
  }
})()
"""

CONSOLE = """
(async () => {
  const host = document.querySelector('graph-studio');
  if (host === null || host.studio === null) {
    return { ok: false, error: { title: 'NoStudio', detail: 'the page has no studio on it' } };
  }
  try {
    const entry = await host.studio.run(%line%);
    return {
      ok: entry.ok === true, message: entry.message,
      error: entry.error === null ? null : { title: entry.error.title, detail: entry.error.detail },
    };
  } catch (failure) {
    return { ok: false, error: { title: 'Threw', detail: String(failure && failure.message || failure) } };
  }
})()
"""

LEGEND = """
(() => {
  const host = document.querySelector('graph-studio');
  const rows = (host === null || host.shadowRoot === null)
    ? [] : Array.from(host.shadowRoot.querySelectorAll('.gs-legend-row'));
  return rows.map((entry) => {
    const swatch = entry.querySelector('.gs-swatch');
    const label = entry.querySelector('.gs-label');
    const count = entry.querySelector('.gs-count');
    return {
      swatch: swatch === null ? '' : getComputedStyle(swatch).backgroundColor,
      label: label === null ? '' : label.textContent,
      count: count === null ? '' : count.textContent,
    };
  });
})()
"""


def _js(template, **values):
    """A JS expression, every `%name%` replaced by a JSON literal of that value."""
    for name, value in values.items():
        template = template.replace(f"%{name}%", json.dumps(value))
    return template


def state(studio):
    report = studio.page.evaluate(STATE)
    if not isinstance(report, dict):
        raise Missing("the page carries no graph-studio with a studio and a view on it")
    return report


def need(studio, *actions):
    """Every named control has to be in the registry, or the row is left unrun with the gap."""
    held = {entry["id"]: entry for entry in state(studio)["actions"]}
    aliases = {entry["alias"] for entry in state(studio)["actions"]}
    gaps = [name for name in actions if name not in held and name not in aliases]
    if gaps:
        raise Missing("the registry has no " + ", ".join(f"`{gap}`" for gap in gaps)
                      + " action yet, so this row had nothing to drive")


def dispatch(studio, action, **args):
    return studio.page.evaluate(_js(DISPATCH, action=action, args=args))


def console(studio, line):
    """The console line a user would type, run through the console's own parser."""
    return studio.page.evaluate(_js(CONSOLE, line=line))


def drive(studio, action, **args):
    """A dispatch that has to have worked; a refusal the studio meant is a FAIL, not a gap."""
    answer = dispatch(studio, action, **args)
    if answer.get("error") is not None:
        failure = answer["error"]
        if failure["title"] in ("NoSuchAction", "NoStudio", "UnknownParam"):
            raise Missing(f"`{action}` could not be driven: {failure['detail']}")
        raise Unjudgeable(f"`{action}` refused: {failure['detail']}")
    return answer


def settled(studio, seconds=BUSY_S):
    """Wait for the motor: a filter row read against a layout still running is a lie."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        busy = studio.page.evaluate(
            "document.querySelector('graph-studio').studio.store.get().busy.length")
        if busy == 0:
            break
        time.sleep(0.2)
    time.sleep(SETTLE_S)


def legend(studio):
    return studio.page.evaluate(LEGEND)
