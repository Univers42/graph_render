/**
 * The schema of the layout that ran, asked once per layout. A refusal is a note and not a
 * failed run: the drawing is the one that was asked for, and the panel says it has no schema.
 */
import type { MotorClient } from "../motor/client.ts";
import { describeError } from "../state/errors.ts";
import type { StudioState } from "../state/model.ts";
import type { Store } from "../state/store.ts";

export async function schemaOf(client: MotorClient, store: Store<StudioState>, layoutId: string): Promise<string[]> {
  if (store.get().schemas[layoutId] !== undefined) return [];
  try {
    const specs = await client.params(layoutId);
    store.update((state) => ({ ...state, schemas: { ...state.schemas, [layoutId]: specs } }));
    return specs.length === 0 ? [`${layoutId} publishes no parameters`] : [];
  } catch (error) {
    return [`the parameters of ${layoutId} are unknown: ${describeError(error).detail}`];
  }
}
