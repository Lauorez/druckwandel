import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  emptyCorrectionMemory,
  parseCorrectionMemory,
  type CorrectionMemory,
} from "../../../src/learning/correction-memory.js";

const BROWSER_STORAGE_KEY = "erechnung-correction-memory-v1";

export async function loadLearningMemory(): Promise<CorrectionMemory> {
  try {
    const contents = isTauri()
      ? await invoke<string | null>("read_learning_memory")
      : window.localStorage.getItem(BROWSER_STORAGE_KEY);
    return parseCorrectionMemory(contents);
  } catch (reason) {
    console.error(reason);
    return emptyCorrectionMemory();
  }
}

export async function saveLearningMemory(memory: CorrectionMemory): Promise<void> {
  const contents = JSON.stringify(memory);
  if (isTauri()) {
    await invoke("write_learning_memory", { contents });
    return;
  }
  window.localStorage.setItem(BROWSER_STORAGE_KEY, contents);
}
