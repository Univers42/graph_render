/**
 * The control column: what to draw, which layout, what the last run cost, the
 * two stage overlays, and the failure banner's host. Presentational — every
 * intent goes back up to `App`.
 */

import type { IngestState } from "../studio/useIngest.ts";
import type { RunnerState } from "../studio/useRunner.ts";
import type { MotorState } from "../studio/useMotorSession.ts";
import type { StagesState } from "../studio/useStages.ts";
import { AnalysisPanel } from "./AnalysisPanel.tsx";
import { DataPanel } from "./DataPanel.tsx";
import { LayoutPanel } from "./LayoutPanel.tsx";
import { PostPanel } from "./PostPanel.tsx";
import { StatusPanel } from "./StatusPanel.tsx";

export interface StudioSideProps {
  readonly ingest: IngestState;
  readonly runner: RunnerState;
  readonly motor: MotorState;
  readonly stages: StagesState;
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
      <PostPanel
        posts={props.motor.posts}
        postId={props.stages.postId}
        report={props.runner.run}
        available={props.motor.available}
        compare={props.runner.compare}
        onPick={props.stages.setPostId}
        onApply={props.stages.applyPost}
        onClear={props.stages.clearPost}
      />
      <AnalysisPanel
        analyses={props.motor.analyses}
        analysisId={props.stages.analysisId}
        result={props.stages.analysis}
        nodeCount={props.runner.run?.nodeCount ?? 0}
        available={props.motor.available}
        onPick={props.stages.setAnalysisId}
        onRun={props.stages.runAnalysis}
        onClear={props.stages.clearAnalysis}
      />
    </aside>
  );
}
