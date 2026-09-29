/**
 * The studio shell. All the state lives in three hooks (motor, ingest, runs) and
 * one failure sink; this file is the wiring and nothing else, so what the studio
 * is made of is readable in one screen.
 */

import { useFailureSink } from "./studio/useFailureSink.ts";
import { useIngest } from "./studio/useIngest.ts";
import { useMotorSession } from "./studio/useMotorSession.ts";
import { useRunner } from "./studio/useRunner.ts";
import { useStages } from "./studio/useStages.ts";
import { StudioMain } from "./ui/StudioMain.tsx";
import { StudioSide } from "./ui/StudioSide.tsx";

function StudioHeader(): React.JSX.Element {
  return (
    <header className="studio__header">
      <h1>graph-motor studio</h1>
      <p className="studio__sub">
        provisional ingest in, wasm motor out, aurora-glass canvas between — every layout the
        module registers, on the graph you give it
      </p>
    </header>
  );
}

export function App(): React.JSX.Element {
  const { errors, fail, clear } = useFailureSink();
  const motor = useMotorSession(fail);
  const ingest = useIngest(fail);
  const runner = useRunner({
    session: motor.session,
    doc: ingest.doc,
    layouts: motor.layouts,
    available: motor.available,
    generation: ingest.generation,
    fail,
  });
  // The two stage overlays act on the SAME handle the runner built, so they are
  // wired to the run on screen rather than owning a graph of their own.
  const stages = useStages({
    session: motor.session,
    run: runner.run,
    onRun: runner.replaceRun,
    generation: ingest.generation,
    fail,
  });

  return (
    <div className="studio">
      <StudioHeader />
      <div className="studio__body">
        <StudioSide ingest={ingest} runner={runner} motor={motor} stages={stages} />
        <StudioMain
          ingest={ingest}
          runner={runner}
          fills={stages.fills}
          analysis={stages.analysis}
          degraded={motor.degraded}
          errors={errors}
          onDismiss={clear}
        />
      </div>
    </div>
  );
}
