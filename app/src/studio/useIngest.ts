/**
 * The ingest document on screen, and the three ways it gets there: regenerated
 * from the seed, read from a file, or normalised from one of the engine's
 * fixtures. A refusal from the normaliser is reported, never half-applied.
 */

import { useCallback, useEffect, useState } from "react";

import { type IngestDoc, normaliseIngest } from "../core/ingestText.ts";
import { syntheticIngest, type SyntheticSpec } from "../core/synthetic.ts";
import type { DataSource } from "../ui/DataPanel.tsx";

/** The ingest the studio opens on: small enough to read, varied enough to show
 *  several node kinds, and the same bytes every time (a seeded generator). */
export const OPENING_SPEC: SyntheticSpec = { seed: 1, nodeCount: 120, degree: 3 };

/** Parse a generated document. It is this module's own `JSON.stringify` output,
 *  so a parse failure here would be a studio bug, and it is reported as one. */
function parseGenerated(spec: SyntheticSpec): IngestDoc {
  return JSON.parse(syntheticIngest(spec)) as IngestDoc;
}

export interface IngestState {
  readonly doc: IngestDoc | null;
  readonly source: DataSource;
  readonly spec: SyntheticSpec;
  /** Bumped on every new document, so a dependent effect can tell "same graph"
   *  from "a different graph" without diffing the document. */
  readonly generation: number;
  readonly setSpec: (spec: SyntheticSpec) => void;
  readonly generate: (spec: SyntheticSpec) => void;
  readonly loadFile: (name: string, text: string) => void;
}

type Adopt = (doc: IngestDoc, source: DataSource) => void;

/** The two ways a document is replaced: regenerated from a seed, or normalised
 *  out of a file. Both report a refusal instead of applying half of it. */
function useLoaders(adopt: Adopt, fail: (error: unknown) => void): Pick<IngestState, "generate" | "loadFile"> {
  const generate = useCallback(
    (next: SyntheticSpec) => {
      try {
        adopt(parseGenerated(next), { name: `synthetic seed ${next.seed}`, notes: [] });
      } catch (error) {
        fail(error);
      }
    },
    [adopt, fail],
  );
  const loadFile = useCallback(
    (name: string, text: string) => {
      try {
        const result = normaliseIngest(text, name);
        adopt(result.doc, { name, notes: [...result.notes] });
      } catch (error) {
        fail(error);
      }
    },
    [adopt, fail],
  );
  return { generate, loadFile };
}

/** The document, its provenance, and the loaders that replace it. */
export function useIngest(fail: (error: unknown) => void): IngestState {
  const [doc, setDoc] = useState<IngestDoc | null>(null);
  const [source, setSource] = useState<DataSource>({ name: "synthetic seed 1", notes: [] });
  const [spec, setSpec] = useState<SyntheticSpec>(OPENING_SPEC);
  const [generation, setGeneration] = useState(0);

  const adopt = useCallback<Adopt>((next, from) => {
    setDoc(next);
    setSource(from);
    setGeneration((previous) => previous + 1);
  }, []);

  // The opening graph, so the studio has something to show before any click.
  useEffect(() => {
    try {
      adopt(parseGenerated(OPENING_SPEC), { name: `synthetic seed ${OPENING_SPEC.seed}`, notes: [] });
    } catch (error) {
      fail(error);
    }
  }, [adopt, fail]);

  return { doc, source, spec, generation, setSpec, ...useLoaders(adopt, fail) };
}
