/**
 * What the last run cost and produced: the two durations, the counts, the node and
 * edge geometry kind, and every column the run carried (or did not).
 */

import type { RunReport } from "../motor/session.ts";

function ms(value: number): string {
  return value < 1 ? `${value.toFixed(2)} ms` : `${value.toFixed(1)} ms`;
}

function Figure(props: { label: string; value: string | number }): React.JSX.Element {
  return (
    <div>
      <dt>{props.label}</dt>
      <dd>{props.value}</dd>
    </div>
  );
}

function ColumnList(props: { report: RunReport }): React.JSX.Element {
  return (
    <details className="notes">
      <summary>columns</summary>
      <ul className="columns">
        {props.report.columns.map((column) => (
          <li key={column.name} className={column.length === null ? "absent" : ""}>
            <code>{column.name}</code> {column.length === null ? "absent" : column.length}
          </li>
        ))}
      </ul>
    </details>
  );
}

function RunCard(props: { report: RunReport; title: string }): React.JSX.Element {
  const { report } = props;
  return (
    <div className="run">
      <h3>
        {props.title} <code>{report.layoutId}</code>
      </h3>
      <dl>
        <Figure label="build" value={ms(report.buildMs)} />
        <Figure label="layout" value={ms(report.layoutMs)} />
        <Figure label="nodes" value={report.nodeCount} />
        <Figure label="edges" value={report.edgeCount} />
        <Figure label="node geometry" value={report.nodeKind} />
        <Figure label="edge geometry" value={report.edgeKind} />
      </dl>
      <ColumnList report={report} />
    </div>
  );
}

export function StatusPanel(props: { run: RunReport | null; second: RunReport | null }): React.JSX.Element {
  if (props.run === null) {
    return (
      <section className="panel-card">
        <h2>Status</h2>
        <p className="hint">nothing has run yet — pick a layout and press Run</p>
      </section>
    );
  }
  return (
    <section className="panel-card">
      <h2>Status</h2>
      <RunCard report={props.run} title="primary" />
      {props.second !== null && <RunCard report={props.second} title="second" />}
    </section>
  );
}
