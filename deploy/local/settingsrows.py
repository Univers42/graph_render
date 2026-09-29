"""The settings rows: persistence per source, a storage that throws, export/import, reset, `?`."""
import json

import cdp
from localrows import CHAIN, CLUSTERED
from navrows import row

SECTIONS = ("appearance", "filter", "layout", "edges", "analysis")
THROW = """
Object.defineProperty(window, 'localStorage', { get() { throw new DOMException('blocked', 'SecurityError'); } });
"""


def change(studio, scale, labels):
    studio.dispatch("appearance.scale", {"factor": scale})
    studio.dispatch("appearance.labels", {"mode": labels})


def look(studio):
    look = studio.settings()["appearance"]
    return (look["nodeScale"], look["labels"])


def row_persist(studio):
    studio.page.evaluate("localStorage.clear()")
    studio.open_source(CLUSTERED)
    change(studio, 1.5, "more")
    studio.open_source(CHAIN)
    change(studio, 2.5, "none")
    studio.open()
    second = (studio.settings()["source"].get("path"), *look(studio))
    studio.open_source(CLUSTERED)
    first = look(studio)
    return row("settings-persist", "two settings survive a reload for their source, and a second source keeps its own",
               f"after reload {second}; back on the first {first}",
               second == (CHAIN, 2.5, "none") and first == (1.5, "more"))


def row_throwing(studio):
    studio.page.call("Page.enable")
    added = studio.page.call("Page.addScriptToEvaluateOnNewDocument", {"source": THROW})
    try:
        try:
            studio.open()
        except cdp.CdpError as failure:
            return row("settings-throwing", "a localStorage that throws leaves the studio open on the defaults",
                       f"did not open: {failure}", False)
        shown = studio.settings()
        loaded = studio.stats()["nodes"] > 0
    finally:
        studio.page.call("Page.removeScriptToEvaluateOnNewDocument", {"identifier": added["identifier"]})
    return row("settings-throwing", "a localStorage that throws leaves the studio open on the defaults",
               f"{shown['source'].get('shape')} source, scale {shown['appearance']['nodeScale']}, "
               f"{studio.stats()['nodes']} nodes", loaded and shown["appearance"]["nodeScale"] == 1)


def exported(studio):
    studio.dispatch("settings.export")
    return studio.last_saved()


def row_roundtrip(studio):
    studio.open()
    studio.capture_saves()
    studio.open_source(CLUSTERED)
    change(studio, 1.5, "more")
    studio.dispatch("filter.degree", {"min": 1})
    before = exported(studio)
    for section in SECTIONS:
        studio.dispatch(f"settings.reset.{section}")
    reset = exported(studio)
    entry = studio.dispatch("settings.import", {"text": before})
    after = exported(studio)
    return row("settings-roundtrip", "export, reset, import gives a document byte-identical to the export",
               f"{len(before)} bytes; reset differs {reset != before}; import ok {entry['ok']}",
               entry["ok"] and reset != before and after == before)


def row_reset_section(studio):
    for section in SECTIONS:
        studio.dispatch(f"settings.reset.{section}")
    baseline = json.loads(exported(studio))
    change(studio, 2, "none")
    studio.dispatch("filter.degree", {"min": 1})
    changed = json.loads(exported(studio))
    studio.dispatch("settings.reset.appearance")
    after = json.loads(exported(studio))
    others = {key: value for key, value in after.items() if key != "appearance"}
    kept = {key: value for key, value in changed.items() if key != "appearance"}
    return row("settings-reset-section", "resetting one panel restores its keys and touches no other",
               f"appearance restored {after['appearance'] == baseline['appearance']}; "
               f"other keys unchanged {others == kept}; filter kept {after['filter'] == changed['filter'] != baseline['filter']}",
               after["appearance"] == baseline["appearance"] != changed["appearance"]
               and others == kept and changed["filter"] != baseline["filter"])


LAST_LOG = """(() => {
  const s = document.querySelector('graph-studio').studio;
  const last = s.store.get().log.at(-1);
  const action = last === undefined ? undefined : s.registry.actions.find(a => last.command.split(' ')[0] === a.alias);
  return { seq: last === undefined ? 0 : last.seq, id: action === undefined ? null : action.id };
})()"""


def overlay_rows(studio):
    return studio.page.evaluate("""
    [...document.querySelector('graph-studio').shadowRoot.querySelectorAll('[data-keymap] tr[data-key]')]
      .map(tr => ({ key: tr.dataset.key, id: tr.dataset.action }))
    """)


def ran(studio, key):
    """The action id the newest log entry names after `key` is pressed, or None if nothing ran."""
    before = studio.page.evaluate(LAST_LOG)
    studio.key(key)
    after = studio.page.evaluate(LAST_LOG)
    return after["id"] if after["seq"] > before["seq"] else None


def row_overlay(studio):
    studio.open()
    studio.focus_page()
    studio.press("?", "Slash", 191, shift=True)
    rows = overlay_rows(studio)
    known = set(studio.page.evaluate("document.querySelector('graph-studio').studio.registry.actions.map(a => a.id)"))
    studio.key("Escape")
    closed = overlay_rows(studio) == []
    wrong = [f"{item['key']}->{item['id']}" for item in rows if item["id"] in known and ran(studio, item["key"]) != item["id"]]
    unregistered = [item["id"] for item in rows if item["id"] not in known]
    return row("overlay-keys", "? lists the key bindings, Escape closes it, and each registered binding runs the action it names",
               f"{len(rows)} rows, {len(rows) - len(unregistered)} registered, chrome-owned {unregistered}, "
               f"closed {closed}, wrong {wrong}",
               closed and not wrong and len(rows) - len(unregistered) >= 11 and len(unregistered) == 3)


def run_rows(studio):
    return [row_persist(studio), row_throwing(studio), row_roundtrip(studio), row_reset_section(studio), row_overlay(studio)]
