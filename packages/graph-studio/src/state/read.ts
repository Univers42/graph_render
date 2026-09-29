/**
 * Reading a document that came from outside (a recipe, a host message) member by member.
 * Every refusal names the path of the member that was wrong.
 */

export type Fields = Readonly<Record<string, unknown>>;

export class SettingsRefusal extends Error {
  constructor(at: string, message: string) {
    super(`${at}: ${message}`);
    this.name = "SettingsRefusal";
  }
}

export function fieldsOf(value: unknown, at: string, members: readonly string[]): Fields {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new SettingsRefusal(at, "not an object");
  const fields: Fields = { ...value };
  for (const key of Object.keys(fields)) {
    if (!members.includes(key)) throw new SettingsRefusal(`${at}.${key}`, "not a member");
  }
  return fields;
}

export function textOf(fields: Fields, at: string, key: string): string {
  const value = fields[key];
  if (typeof value !== "string") throw new SettingsRefusal(`${at}.${key}`, "not a string");
  return value;
}

export function textOrNull(fields: Fields, at: string, key: string): string | null {
  return fields[key] === null ? null : textOf(fields, at, key);
}

export interface Range {
  readonly min: number;
  readonly max: number;
  readonly whole: boolean;
}

export function numberOf(fields: Fields, at: string, key: string, range: Range): number {
  const value = fields[key];
  const fits = typeof value === "number" && Number.isFinite(value) && (!range.whole || Number.isInteger(value));
  if (!fits || value < range.min || value > range.max) {
    throw new SettingsRefusal(`${at}.${key}`, `not a ${range.whole ? "whole " : ""}number in ${range.min}..${range.max}`);
  }
  return value;
}

export function oneOf<Choice extends string>(fields: Fields, at: string, key: string, choices: readonly Choice[]): Choice {
  const value = fields[key];
  const found = choices.find((choice) => choice === value);
  if (found === undefined) throw new SettingsRefusal(`${at}.${key}`, `not one of ${choices.join(", ")}`);
  return found;
}

export function textsOf(fields: Fields, at: string, key: string): readonly string[] {
  const value: unknown = fields[key];
  if (!Array.isArray(value)) throw new SettingsRefusal(`${at}.${key}`, "not a list");
  return value.map((item: unknown, i) => {
    if (typeof item !== "string") throw new SettingsRefusal(`${at}.${key}[${i}]`, "not a string");
    return item;
  });
}

export function flagOf(fields: Fields, at: string, key: string): boolean {
  const value = fields[key];
  if (typeof value !== "boolean") throw new SettingsRefusal(`${at}.${key}`, "not true or false");
  return value;
}
