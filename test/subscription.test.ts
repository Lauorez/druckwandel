import { expect, it, vi } from "vitest";
import { subscription } from "../apps/desktop/src/subscription.js";

it("releases an event listener whose registration resolves after unmount", async () => {
  let register!: (stop: () => void) => void;
  const pending = new Promise<() => void>(resolve => { register = resolve; });
  const stop = vi.fn();
  const dispose = subscription(pending);
  dispose();
  register(stop);
  await pending;
  expect(stop).toHaveBeenCalledOnce();
});
