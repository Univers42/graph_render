/**
 * The layout picker. Its list comes from `Motor.layouts()` — the module's own
 * registry, in registry order — so a layout registered after this file was
 * written appears here with no change to the studio (C1). Nothing in this file
 * names a layout.
 */

export interface LayoutPanelProps {
  readonly layouts: readonly string[];
  readonly layoutId: string | null;
  readonly compareId: string | null;
  readonly compare: boolean;
  readonly available: boolean;
  readonly onRun: (layoutId: string, compareId: string | null) => void;
  readonly onCompareId: (layoutId: string) => void;
  readonly onCompare: (on: boolean) => void;
}

interface PickerProps {
  readonly label: string;
  readonly layouts: readonly string[];
  readonly value: string;
  readonly onChange: (id: string) => void;
}

function Picker(props: PickerProps): React.JSX.Element {
  return (
    <label className="field">
      <span>{props.label}</span>
      <select
        value={props.value}
        disabled={props.layouts.length === 0}
        onChange={(event) => props.onChange(event.target.value)}
      >
        {props.layouts.length === 0 && <option value="">(no layouts — motor unavailable)</option>}
        {props.layouts.map((id) => (
          <option key={id} value={id}>
            {id}
          </option>
        ))}
      </select>
    </label>
  );
}

/** Which layouts the two slots hold right now. The second one defaults to the
 *  first registered layout that is not the first slot's, so compare mode is
 *  useful the moment it is switched on. */
function slots(props: LayoutPanelProps): { first: string; second: string } {
  const first = props.layoutId ?? props.layouts[0] ?? "";
  const fallback = props.layouts.find((id) => id !== first) ?? "";
  return { first, second: props.compareId ?? fallback };
}

/** The second slot and its own Run: one handle, two layouts, one click. */
function CompareSlot(props: LayoutPanelProps & { first: string; second: string; ready: boolean }): React.JSX.Element {
  return (
    <>
      <Picker label="second" layouts={props.layouts} value={props.second} onChange={props.onCompareId} />
      <button
        type="button"
        className="secondary"
        disabled={!props.ready || props.second === ""}
        onClick={() => props.onRun(props.first, props.second)}
      >
        Run both
      </button>
    </>
  );
}

export function LayoutPanel(props: LayoutPanelProps): React.JSX.Element {
  const { first, second } = slots(props);
  const ready = props.available && first !== "";
  return (
    <section className="panel-card">
      <h2>Layout</h2>
      <Picker
        label="primary"
        layouts={props.layouts}
        value={first}
        onChange={(id) => props.onRun(id, props.compare ? second : null)}
      />
      <div className="row">
        <button type="button" disabled={!ready} onClick={() => props.onRun(first, props.compare ? second : null)}>
          Run
        </button>
        <label className="check">
          <input
            type="checkbox"
            checked={props.compare}
            disabled={props.layouts.length < 2}
            onChange={(event) => props.onCompare(event.target.checked)}
          />
          compare
        </label>
      </div>
      {props.compare && (
        <CompareSlot {...props} first={first} second={second} ready={ready} />
      )}
      <p className="hint">{props.layouts.length} layout(s) registered by the module</p>
    </section>
  );
}
