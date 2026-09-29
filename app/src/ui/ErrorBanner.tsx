/**
 * Failures, on the page. A degraded motor gets a standing banner (nothing can
 * run), a refusal gets a dismissible card, and neither ever replaces the studio:
 * the panels stay, the graph stays, the reason is right there.
 */

import type { ShownError } from "../core/errors.ts";

export interface ErrorBannerProps {
  readonly degraded: ShownError | null;
  readonly errors: readonly ShownError[];
  readonly onDismiss: () => void;
}

function Card(props: { error: ShownError; onDismiss?: () => void }): React.JSX.Element {
  return (
    <div className="error" role="alert">
      <div className="error__head">
        <strong>{props.error.title}</strong>
        {props.error.code !== null && <code className="error__code">{props.error.code}</code>}
        {props.onDismiss !== undefined && (
          <button type="button" className="error__close" onClick={props.onDismiss}>
            dismiss
          </button>
        )}
      </div>
      <p className="error__detail">{props.error.detail}</p>
      <p className="error__hint">{props.error.hint}</p>
    </div>
  );
}

export function ErrorBanner(props: ErrorBannerProps): React.JSX.Element | null {
  if (props.degraded === null && props.errors.length === 0) return null;
  return (
    <div className="banners">
      {props.degraded !== null && (
        <div className="error error--degraded" role="alert">
          <div className="error__head">
            <strong>motor degraded</strong>
            <code className="error__code">{props.degraded.title}</code>
          </div>
          <p className="error__detail">{props.degraded.detail}</p>
          <p className="error__hint">{props.degraded.hint}</p>
        </div>
      )}
      {props.errors.map((error, index) => (
        <Card key={`${error.title}-${index}`} error={error} onDismiss={props.onDismiss} />
      ))}
    </div>
  );
}
