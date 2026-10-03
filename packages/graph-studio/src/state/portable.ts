/** Settings as a document that leaves the studio and comes back unchanged. */
import { DEFAULT_SETTINGS, type Settings, readSettings, withSettings } from "./settings.ts";

/** The dock panels that keep their own settings member, one each. */
export const SECTIONS = ["appearance", "filter", "layout", "edges", "analysis", "params"] as const;

export type Section = (typeof SECTIONS)[number];

/** Members in the settings' fixed order and two-space indent: equal settings are equal bytes. */
export function exportSettings(settings: Settings): string {
  return JSON.stringify(settings, null, 2);
}

export function importSettings(text: string): Settings {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("the settings file is not JSON");
  }
  return readSettings(value);
}

/** `settings` with the member `section` owns put back to its default and nothing else touched. */
export function resetSection(settings: Settings, section: Section): Settings {
  return withSettings(settings, { [section]: DEFAULT_SETTINGS[section] });
}
