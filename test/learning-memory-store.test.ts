import { expect, it, vi } from "vitest";
import { emptyCorrectionMemory } from "../src/learning/correction-memory.js";
import { saveLearningMemory } from "../apps/desktop/src/learningMemoryStore.js";

const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: native.invoke }));

it("does not replace stored profiles with defaults after a read failure", async () => {
  native.invoke.mockRejectedValueOnce(new Error("read failed"));
  await expect(saveLearningMemory(emptyCorrectionMemory())).rejects.toThrow("Erkennungsprofile konnten nicht geladen");
  expect(native.invoke).toHaveBeenCalledExactlyOnceWith("read_learning_memory");
});
