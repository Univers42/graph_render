/** The navigation bar in the HUD: the camera's buttons, each one an action of the studio. */
import type { ReactElement } from "react";

import type { Studio } from "../studio/studio.ts";
import { ARROW_PAN, ZOOM_STEP } from "./navKeys.ts";

export interface NavBarProps {
  readonly studio: Studio;
}

interface Button {
  readonly label: string;
  readonly title: string;
  readonly id: string;
  readonly args: Readonly<Record<string, number>>;
}

/**
 * The five buttons, in the order the eye reads them. `fit` and `reset` are the two that
 * answer "where am I"; zoom and the arrow step are what a trackpad reaches for.
 *
 * The lock button is not here: it freezes the motor, which is S6, and a button that
 * promised it would be a lie until then.
 */
const BUTTONS: readonly Button[] = [
  { label: "⤢", title: "Fit the graph to the view", id: "view.fit", args: {} },
  { label: "+", title: `Zoom in ×${ZOOM_STEP}`, id: "view.zoom", args: { factor: ZOOM_STEP } },
  { label: "−", title: `Zoom out ÷${ZOOM_STEP}`, id: "view.zoom", args: { factor: 1 / ZOOM_STEP } },
  { label: "0", title: "Reset the camera to 1:1", id: "view.reset", args: {} },
  { label: `←${ARROW_PAN}→`, title: `Pan ${ARROW_PAN} pixels`, id: "view.pan", args: { dx: ARROW_PAN, dy: 0 } },
];

export function NavBar(props: NavBarProps): ReactElement {
  const { studio } = props;
  return (
    <div className="gs-nav" role="group" aria-label="Camera">
      {BUTTONS.map((button) => (
        <button
          key={button.label}
          type="button"
          className="gs-btn gs-nav-btn"
          aria-label={button.title}
          title={button.title}
          onClick={() => void studio.dispatch(button.id, { ...button.args })}
        >
          {button.label}
        </button>
      ))}
    </div>
  );
}
