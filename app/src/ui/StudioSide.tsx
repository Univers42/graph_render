/**
 * The control column: what to draw, which layout, what the last run cost, and the
 * reserved slot for the post/analysis branch. Presentational — every intent goes
 * back up to `App`.
 */

import type { IngestState } from "../studio/useIngest.ts";
import type { RunnerState } from "../studio/useRunner.ts";
import type { MotorState } from "../studio/useMotorSession.ts";
import { DataPanel } from "./DataPanel.tsx";
import { LayoutPanel } from "./LayoutPanel.tsx";
import { PostSlot } from "./PostSlot.tsx";
import { StatusPanel } from "./StatusPanel.tsx";

export interface StudioSideProps {
  readonly ingest: IngestState;
  readonly runner: RunnerState;
  readonly motor: MotorState;
}

export function StudioSide(props: StudioSideProps): React.JSX.Element {
  return (
    <aside className="studio__side">
      <DataPanel
        spec={props.ingest.spec}
        source={props.ingest.source}
        onSpec={props.ingest.setSpec}
        onSynthetic={props.ingest.generate}
        onFile={props.ingest.loadFile}
      />
      <LayoutPanel
        layouts={props.motor.layouts}
        layoutId={props.runner.layoutId}
        compareId={props.runner.compareId}
        compare={props.runner.compare}
        available={props.motor.available}
        onRun={props.runner.runLayouts}
        onCompareId={props.runner.setCompareId}
        onCompare={props.runner.setCompare}
      />
      <StatusPanel run={props.runner.run} second={props.runner.compare ? props.runner.second : null} />
      <PostSlot session={props.motor.session} available={props.motor.available} />
    </aside>
  );
}
