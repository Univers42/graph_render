/** What the node menu offers, apart from how it is drawn. */
export type MenuItem = "focus" | "pin" | "hide" | "copy";

export interface MenuEntry {
  readonly item: MenuItem;
  readonly label: string;
}

/** The pin entry reads as its own undo once the node is pinned. */
export function entriesFor(pinned: boolean): readonly MenuEntry[] {
  return [
    { item: "focus", label: "Focus" },
    { item: "pin", label: pinned ? "Unpin" : "Pin" },
    { item: "hide", label: "Hide" },
    { item: "copy", label: "Copy id" },
  ];
}

/** Arrow keys wrap; Home and End go to the ends; anything else stays. */
export function nextEntry(at: number, key: string, count: number): number {
  if (key === "ArrowDown") return (at + 1) % count;
  if (key === "ArrowUp") return (at - 1 + count) % count;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  return at;
}
