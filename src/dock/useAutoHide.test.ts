import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const dockReveal = vi.fn(async (_revealed: boolean) => {});
let revealedListener: ((revealed: boolean) => void) | undefined;
vi.mock("../core/ipc", () => ({
  ipc: {
    dockReveal: (revealed: boolean) => dockReveal(revealed),
    dockIsRevealed: async () => false,
  },
  events: {
    onDockRevealed: async (handler: (revealed: boolean) => void) => {
      revealedListener = handler;
      return () => {};
    },
  },
}));

const { HIDE_DELAY_MS, useAutoHide } = await import("./useAutoHide");

describe("useAutoHide", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    dockReveal.mockClear();
  });
  afterEach(() => vi.useRealTimers());

  it("is always revealed when auto-hide is off", () => {
    const { result } = renderHook(() => useAutoHide(false));
    act(() => result.current.onEnter());
    expect(result.current.revealed).toBe(true);
    expect(dockReveal).not.toHaveBeenCalled();
  });

  it("reveals on enter and tucks away a moment after leaving", () => {
    const { result } = renderHook(() => useAutoHide(true));
    act(() => result.current.onEnter());
    expect(dockReveal).toHaveBeenLastCalledWith(true);
    act(() => result.current.onLeave());
    act(() => vi.advanceTimersByTime(HIDE_DELAY_MS - 1));
    expect(dockReveal).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(1));
    expect(dockReveal).toHaveBeenLastCalledWith(false);
  });

  it("coming back before the delay keeps it out", () => {
    const { result } = renderHook(() => useAutoHide(true));
    act(() => result.current.onLeave());
    act(() => vi.advanceTimersByTime(HIDE_DELAY_MS / 2));
    act(() => result.current.onEnter());
    act(() => vi.advanceTimersByTime(HIDE_DELAY_MS * 2));
    expect(dockReveal).not.toHaveBeenCalledWith(false);
  });

  it("follows what the backend reports", async () => {
    const { result } = renderHook(() => useAutoHide(true));
    await act(async () => {});
    expect(result.current.revealed).toBe(false);
    act(() => revealedListener?.(true));
    expect(result.current.revealed).toBe(true);
  });
});
