import { sample } from "./sampleData";

const state = new URLSearchParams(window.location.search).get("state") ?? "full";

export const featuresIpc = new Proxy(
  {},
  {
    get: (_t, name: string) => () =>
      state === "fail"
        ? Promise.reject(new Error(`${name} failed (gallery)`))
        : Promise.resolve(
            state === "empty" ? (Array.isArray(sample[name]) ? [] : null) : (sample[name] ?? null),
          ),
  },
) as never;

const noop = () => Promise.resolve(() => {});
export const featureEvents = new Proxy({}, { get: () => noop }) as never;

export function errorText(error: unknown, fallback = "Something went wrong. Try again."): string {
  const kind = (error as { kind?: string })?.kind;
  const message = (error as { message?: string })?.message ?? "";
  return kind === "invalid_input" ? message.replace(/^invalid input: /, "") : fallback;
}

export function subscribe(start: () => Promise<() => void>): () => void {
  let unlisten: (() => void) | undefined;
  let alive = true;
  start().then((fn) => (alive ? (unlisten = fn) : fn()));
  return () => {
    alive = false;
    unlisten?.();
  };
}
