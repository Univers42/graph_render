/**
 * The studio shell. All the state lives in three hooks (motor, ingest, runs) and
 * one failure sink; this file is the wiring and nothing else, so what the studio
 * is made of is readable in one screen.
 */

import { useFailureSink } from "./studio/useFailureSink.ts";
import { useIngest } from "./studio/useIngest.ts";
import { useMotorSession } from "./studio/useMotorSession.ts";
import { useRunner } from "./studio/useRunner.ts";
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

  return (
    <div className="studio">
      <StudioHeader />
      <div className="studio__body">
        <StudioSide ingest={ingest} runner={runner} motor={motor} />
        <StudioMain
          ingest={ingest}
          runner={runner}
          degraded={motor.degraded}
          errors={errors}
          onDismiss={clear}
        />
      </div>
    </div>
  );
}
