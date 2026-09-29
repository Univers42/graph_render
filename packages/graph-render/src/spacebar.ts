/** Whether the space bar is down, and the target that says so. One flag for the whole page. */

/** What a key event says about the space bar; a `KeyboardEvent` is one. */
export type SpacePress = Pick<KeyboardEvent, "key" | "code" | "repeat">;

const SPACE_KEY = " ";
const SPACE_CODE = "Space";

let down = false;

export function isDown(): boolean {
  return down;
}

/**
 * A key that is not the space bar leaves the flag alone. A repeat is a held key saying
 * itself again, which for one flag is the same answer, so `repeat` is not read here.
 */
export function press(event: SpacePress): void {
  if (event.key !== SPACE_KEY && event.code !== SPACE_CODE) return;
  down = true;
}

export function release(event: SpacePress): void {
  if (event.key !== SPACE_KEY && event.code !== SPACE_CODE) return;
  down = false;
}

/** The window lost the focus with the bar down: nothing will report the release. */
export function forget(): void {
  down = false;
}
