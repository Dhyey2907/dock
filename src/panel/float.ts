import type { WidgetKind } from "../core/generated/WidgetKind";
import { WIDGETS } from "../widgets/registry";

/** The widget a floating window shows, from its address ("#float/clock"). */
export function floatKind(hash: string): WidgetKind | null {
  const kind = hash.replace(/^#float\//, "");
  return kind && kind in WIDGETS ? (kind as WidgetKind) : null;
}
