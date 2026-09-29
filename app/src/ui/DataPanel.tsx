/**
 * Where the graph comes from: a seeded synthetic model, a JSON file from disk, or
 * one of the engine's own fixtures.
 */

import { useState } from "react";

import { FIXTURES } from "../data/fixtures.ts";
import type { SyntheticSpec } from "../core/synthetic.ts";

export interface DataSource {
  readonly name: string;
  /** Every default filled and annotation dropped while normalising. */
  readonly notes: readonly string[];
}

export interface DataPanelProps {
  readonly spec: SyntheticSpec;
  readonly source: DataSource;
  readonly onSpec: (spec: SyntheticSpec) => void;
  readonly onSynthetic: (spec: SyntheticSpec) => void;
  readonly onFile: (name: string, text: string) => void;
}

function NumberField(props: { label: string; value: number; onChange: (next: number) => void }): React.JSX.Element {
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        type="number"
        value={props.value}
        onChange={(event) => props.onChange(Number(event.target.value))}
      />
    </label>
  );
}

function SyntheticFields(props: DataPanelProps): React.JSX.Element {
  const patch = (part: Partial<SyntheticSpec>): void => props.onSpec({ ...props.spec, ...part });
  return (
    <>
      <div className="fields">
        <NumberField label="seed" value={props.spec.seed} onChange={(seed) => patch({ seed })} />
        <NumberField label="nodes" value={props.spec.nodeCount} onChange={(nodeCount) => patch({ nodeCount })} />
        <NumberField label="reference degree" value={props.spec.degree} onChange={(degree) => patch({ degree })} />
      </div>
      <div className="row">
        <button type="button" onClick={() => props.onSynthetic(props.spec)}>
          Generate
        </button>
        <span className="hint">the same seed always builds the same graph</span>
      </div>
    </>
  );
}

function FilePicker(props: { onFile: (name: string, text: string) => void }): React.JSX.Element {
  return (
    <label className="field">
      <span>open a JSON file</span>
      <input
        type="file"
        accept="application/json,.json"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file === undefined) return;
          void file.text().then((text) => props.onFile(file.name, text));
        }}
      />
    </label>
  );
}

function FixturePicker(props: { onFile: (name: string, text: string) => void }): React.JSX.Element {
  const [fixture, setFixture] = useState(FIXTURES[0] ?? "");
  const [status, setStatus] = useState<string | null>(null);

  const load = async (): Promise<void> => {
    setStatus(`loading ${fixture}…`);
    try {
      const response = await fetch(`fixtures/${fixture}`);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      props.onFile(fixture, await response.text());
      setStatus(`loaded ${fixture}`);
    } catch (error) {
      // A missing fixture directory (a bare `vite build`, no `studio.sh`) is a
      // normal state, not a crash: the reason goes in the panel and the studio
      // carries on with whatever is already loaded.
      setStatus(`could not load ${fixture}: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  return (
    <>
      <div className="row">
        <select value={fixture} onChange={(event) => setFixture(event.target.value)}>
          {FIXTURES.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        <button type="button" onClick={() => void load()}>
          Load fixture
        </button>
      </div>
      {status !== null && <p className="hint">{status}</p>}
    </>
  );
}

function SourceNotes(props: { source: DataSource }): React.JSX.Element {
  return (
    <>
      <p className="source">
        in view: <strong>{props.source.name}</strong>
      </p>
      {props.source.notes.length > 0 && (
        <details className="notes">
          <summary>{props.source.notes.length} normalisation note(s)</summary>
          <ul>
            {props.source.notes.slice(0, 12).map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
}

export function DataPanel(props: DataPanelProps): React.JSX.Element {
  return (
    <section className="panel-card">
      <h2>Data</h2>
      <SyntheticFields {...props} />
      <FilePicker onFile={props.onFile} />
      <FixturePicker onFile={props.onFile} />
      <SourceNotes source={props.source} />
    </section>
  );
}
