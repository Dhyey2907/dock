import { create } from "zustand";

import { events, ipc } from "../../core/ipc";
import type { DockGeometry } from "../../core/generated/DockGeometry";
import type { GlassFrame } from "../../core/generated/GlassFrame";

interface GlassState {
  /** The desktop picture behind the dock; null until read (or if it can't be). */
  frame: GlassFrame | null;
  /** Where the dock window sits on its screen. */
  geometry: DockGeometry | null;
  load: () => Promise<void>;
}

export const useGlass = create<GlassState>((set) => ({
  frame: null,
  geometry: null,
  load: async () => {
    // Listen first so a change while loading isn't missed.
    await events.onGlassChanged((frame) => set({ frame }));
    await events.onDockGeometry((geometry) => set({ geometry }));
    const [frame, geometry] = await Promise.all([ipc.glassGet(), ipc.dockGeometry()]);
    set((s) => ({ frame: s.frame ?? frame, geometry: s.geometry ?? geometry }));
  },
}));
