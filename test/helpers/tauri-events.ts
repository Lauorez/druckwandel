import { vi } from "vitest";

/** Models native events without depending on an installed WebView IPC bridge. */
export function tauriEvents() {
  const handlers = new Map<string, Set<(event: { payload: unknown }) => void>>();
  return {
    listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
      const listeners = handlers.get(name) ?? new Set();
      handlers.set(name, listeners);
      listeners.add(handler);
      return () => { listeners.delete(handler); };
    }),
    emit: vi.fn(async (name: string, payload: unknown) => {
      for (const handler of handlers.get(name) ?? []) handler({ payload });
    }),
  };
}
