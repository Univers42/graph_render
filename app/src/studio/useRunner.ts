/**
 * Runs. One handle serves both compare-mode layouts, so the two panels are
 * guaranteed to be the same graph under two layouts rather than two graphs that
 * look alike — the whole point of the comparison.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { IngestDoc } from "../core/ingestText.ts";
import type { MotorSession, RunReport } from "../motor/session.ts";

export interface RunnerState {
  readonly layoutId: string | null;
  readonly compareId: string | null;
  readonly compare: boolean;
  readonly run: RunReport | null;
  readonly second: RunReport | null;
  readonly runLayouts: (primary: string, other: string | null) => void;
  readonly setCompare: (on: boolean) => void;
  readonly setCompareId: (id: string) => void;
  /** Replace the primary run in place — how a POST pass, which runs on the same
   *  handle, hands the canvas its new geometry without a second build. */
  readonly replaceRun: (report: RunReport) => void;
}

export interface RunnerArgs {
  readonly session: MotorSession | null;
  readonly doc: IngestDoc | null;
  readonly layouts: readonly string[];
  readonly available: boolean;
  /** Bumped on every new document: a new graph clears the panels and re-arms the
   *  opening run, so the reader never has to press Run to see what they loaded. */
  readonly generation: number;
  readonly fail: (error: unknown) => void;
}

/** The studio opens ON a graph: once the registry has answered, the first
 *  registered layout runs over whatever document is on screen — once per
 *  document, not once per render. */
function useAutoRun(args: RunnerArgs, run: (id: string) => void): void {
  const done = useRef(-1);
  useEffect(() => {
    if (!args.available || args.layouts.length === 0 || args.doc === null) return;
    if (done.current === args.generation) return;
    done.current = args.generation;
    run(args.layouts[0]);
  }, [args.available, args.doc, args.generation, args.layouts, run]);
}

interface Slots {
  readonly layoutId: string | null;
  readonly compareId: string | null;
  readonly compare: boolean;
  readonly setLayoutId: (id: string) => void;
  readonly setCompareId: (id: string) => void;
  readonly setCompare: (on: boolean) => void;
}

/** The two layout slots and the compare switch, plus the two run results. */
function useSlots(): Slots & { run: RunReport | null; second: RunReport | null; setRun: (r: RunReport | null) => void; setSecond: (r: RunReport | null) => void } {
  const [layoutId, setLayoutId] = useState<string | null>(null);
  const [compareId, setCompareId] = useState<string | null>(null);
  const [compare, setCompare] = useState(false);
  const [run, setRun] = useState<RunReport | null>(null);
  const [second, setSecond] = useState<RunReport | null>(null);
  return { layoutId, compareId, compare, setLayoutId, setCompareId, setCompare, run, second, setRun, setSecond };
}

export function useRunner(args: RunnerArgs): RunnerState {
  const { session, doc, generation, fail } = args;
  const slots = useSlots();
  const { setLayoutId, setCompareId, setCompare, setRun, setSecond } = slots;

  const runLayouts = useCallback(
    (primary: string, other: string | null) => {
      if (session === null || doc === null) return;
      // The picker is controlled by these slots: a run that does not record its
      // ids snaps the select back to the previous layout on the next render.
      setLayoutId(primary);
      if (other !== null) setCompareId(other);
      try {
        const { buildMs } = session.build(JSON.stringify(doc));
        setRun(session.run(primary, buildMs));
        setSecond(other === null ? null : session.run(other, buildMs));
      } catch (error) {
        fail(error);
      }
    },
    [doc, fail, session, setCompareId, setLayoutId],
  );

  // Declared before the auto-run so a new document clears the panels in the same
  // commit that refills them.
  useEffect(() => {
    setRun(null);
    setSecond(null);
    setCompare(false);
  }, [generation]);

  const open = useCallback((id: string) => runLayouts(id, null), [runLayouts]);
  useAutoRun(args, open);

  const replaceRun = useCallback((report: RunReport) => setRun(report), []);

  return {
    layoutId: slots.layoutId, compareId: slots.compareId, compare: slots.compare,
    run: slots.run, second: slots.second,
    runLayouts, setCompare, setCompareId, replaceRun,
  };
}
