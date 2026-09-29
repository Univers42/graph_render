/**
 * The graph area and the failure banner above it. A failure never replaces the
 * studio: the panels stay, the graph stays, the reason is in the banner.
 */

import { useMemo } from "react";

import type { AnalysisResult } from "../../../crates/graph-sdk-js/src/index.ts";
import type { ShownError } from "../core/errors.ts";
import type { IngestDoc } from "../core/ingestText.ts";
import type { IngestState } from "../studio/useIngest.ts";
import type { RunnerState } from "../studio/useRunner.ts";
import { edgeKinds, stylesFor } from "../render/palette.ts";
import { ErrorBanner } from "./ErrorBanner.tsx";
import { GraphPanel } from "./GraphPanel.tsx";

/** The empty projection, for the moment before the first document lands. */
const NO_DOCUMENT: IngestDoc = { version: 1, nodes: [], edges: [] };

export interface StudioMainProps {
  readonly ingest: IngestState;
  readonly runner: RunnerState;
  /** One fill per node from an applied analysis, or `null`. */
  readonly fills: readonly string[] | null;
  readonly analysis: AnalysisResult | null;
  readonly degraded: ShownError | null;
  readonly errors: readonly ShownError[];
  readonly onDismiss: () => void;
}

export function StudioMain(props: StudioMainProps): React.JSX.Element {
  const { doc } = props.ingest;
  // Per-document, not per-frame: the styles are a projection of the ingest, and
  // rebuilding them on every pointer move would be the studio's own hot loop.
  const styles = useMemo(() => stylesFor(doc ?? NO_DOCUMENT), [doc]);
  const kinds = useMemo(() => edgeKinds(doc ?? NO_DOCUMENT), [doc]);

  return (
    <main className="studio__main">
      <ErrorBanner
        degraded={props.degraded}
        errors={props.errors}
        onDismiss={props.onDismiss}
      />
      <GraphPanel
        run={props.runner.run}
        second={props.runner.compare ? props.runner.second : null}
        styles={styles}
        edgeKinds={kinds}
        fills={props.fills}
        analysis={props.analysis}
        nodeCount={doc?.nodes.length ?? 0}
        edgeCount={doc?.edges.length ?? 0}
      />
    </main>
  );
}
