import type { WidgetEntry } from "../core/generated/WidgetEntry";
import { CATEGORY_ORDER, type WidgetCategory, type WidgetDefinition } from "../widgets/registry";
import type { WidgetKind } from "../core/generated/WidgetKind";

export interface GalleryGroup {
  category: WidgetCategory;
  entries: WidgetEntry[];
}

/**
 * Widgets this version can show, grouped by category in a fixed order, keeping the saved
 * order within a group. `query` filters by name or description, ignoring case.
 */
export function galleryGroups(
  widgets: WidgetEntry[],
  defs: Partial<Record<WidgetKind, WidgetDefinition>>,
  query: string,
): GalleryGroup[] {
  const q = query.trim().toLowerCase();
  const matches = (entry: WidgetEntry) => {
    const def = defs[entry.kind];
    if (!def) return false;
    return !q || def.label.toLowerCase().includes(q) || def.description.toLowerCase().includes(q);
  };
  const groups = new Map<WidgetCategory, WidgetEntry[]>();
  for (const entry of widgets.filter(matches)) {
    const category = defs[entry.kind]?.category ?? "info";
    groups.set(category, [...(groups.get(category) ?? []), entry]);
  }
  return CATEGORY_ORDER.filter((c) => groups.has(c)).map((category) => ({
    category,
    entries: groups.get(category)!,
  }));
}
