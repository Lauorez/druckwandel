import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  activeLearningProfile,
  parseLearningProfileStore,
  type LearningProfileStore,
} from "../../../src/learning/profiles.js";
import type { CorrectionMemory } from "../../../src/learning/correction-memory.js";

const BROWSER_STORAGE_KEY = "erechnung-correction-memory-v1";

export async function loadLearningProfiles(): Promise<LearningProfileStore> {
  try {
    const contents = isTauri()
      ? await invoke<string | null>("read_learning_memory")
      : window.localStorage.getItem(BROWSER_STORAGE_KEY);
    return parseLearningProfileStore(contents);
  } catch (reason) {
    console.error(reason);
    return parseLearningProfileStore(null);
  }
}

export async function saveLearningProfiles(store: LearningProfileStore): Promise<void> {
  const contents = JSON.stringify(store);
  if (isTauri()) {
    await invoke("write_learning_memory", { contents });
    return;
  }
  window.localStorage.setItem(BROWSER_STORAGE_KEY, contents);
}

export async function loadLearningMemory(): Promise<CorrectionMemory> {
  return activeLearningProfile(await loadLearningProfiles()).memory;
}

export async function saveLearningMemory(memory: CorrectionMemory): Promise<void> {
  const store = await loadLearningProfiles();
  const { replaceActiveMemory } = await import("../../../src/learning/profiles.js");
  await saveLearningProfiles(replaceActiveMemory(store, memory));
}
