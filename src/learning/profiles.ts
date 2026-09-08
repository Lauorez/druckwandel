import {
  emptyCorrectionMemory,
  parseCorrectionMemory,
  type CorrectionMemory,
} from "./correction-memory.js";

export interface LearningProfile {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  memory: CorrectionMemory;
}

export interface LearningProfileStore {
  schemaVersion: 2;
  activeProfileId: string;
  profiles: LearningProfile[];
}

export const DEFAULT_PROFILE_NAME = "Standard";

function profileId(name: string, now: string): string {
  return `profile-${name}-${now}`.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 80);
}

export function emptyLearningProfileStore(now = new Date().toISOString()): LearningProfileStore {
  const id = "profile-standard";
  return {
    schemaVersion: 2,
    activeProfileId: id,
    profiles: [{ id, name: DEFAULT_PROFILE_NAME, createdAt: now, updatedAt: now, memory: emptyCorrectionMemory() }],
  };
}

function isProfile(value: unknown): value is LearningProfile {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<LearningProfile>;
  return typeof candidate.id === "string" && candidate.id.length > 0
    && typeof candidate.name === "string" && candidate.name.trim().length > 0 && candidate.name.length <= 80
    && typeof candidate.createdAt === "string"
    && typeof candidate.updatedAt === "string"
    && typeof candidate.memory === "object" && candidate.memory !== null;
}

export function parseLearningProfileStore(contents: string | null | undefined): LearningProfileStore {
  if (!contents) return emptyLearningProfileStore();
  try {
    const parsed = JSON.parse(contents) as {
      schemaVersion?: number;
      rules?: unknown;
      activeProfileId?: string;
      profiles?: unknown;
    };
    if (parsed.schemaVersion === 1 || Array.isArray(parsed.rules)) {
      const memory = parseCorrectionMemory(contents);
      const store = emptyLearningProfileStore();
      store.profiles[0]!.memory = memory;
      return store;
    }
    if (parsed.schemaVersion !== 2 || typeof parsed.activeProfileId !== "string" || !Array.isArray(parsed.profiles)) {
      return emptyLearningProfileStore();
    }
    const profiles = parsed.profiles.filter(isProfile).map((profile) => ({
      ...profile,
      name: profile.name.trim(),
      memory: parseCorrectionMemory(JSON.stringify(profile.memory)),
    })).slice(0, 20);
    if (profiles.length === 0) return emptyLearningProfileStore();
    const activeProfileId = profiles.some((profile) => profile.id === parsed.activeProfileId)
      ? parsed.activeProfileId
      : profiles[0]!.id;
    return { schemaVersion: 2, activeProfileId, profiles };
  } catch {
    return emptyLearningProfileStore();
  }
}

export function activeLearningProfile(store: LearningProfileStore): LearningProfile {
  return store.profiles.find((profile) => profile.id === store.activeProfileId) ?? store.profiles[0]!;
}

export function selectLearningProfile(store: LearningProfileStore, profileId: string): LearningProfileStore {
  if (!store.profiles.some((profile) => profile.id === profileId)) return store;
  return { ...store, activeProfileId: profileId };
}

export function createLearningProfile(store: LearningProfileStore, name: string, now = new Date().toISOString()): LearningProfileStore | null {
  const trimmed = name.trim();
  if (!trimmed || store.profiles.length >= 20) return null;
  const id = profileId(`${trimmed}-${now}`, now);
  if (store.profiles.some((profile) => profile.id === id)) return null;
  const profile: LearningProfile = { id, name: trimmed.slice(0, 80), createdAt: now, updatedAt: now, memory: emptyCorrectionMemory() };
  return { ...store, activeProfileId: id, profiles: [...store.profiles, profile] };
}

export function renameLearningProfile(store: LearningProfileStore, profileId: string, name: string, now = new Date().toISOString()): LearningProfileStore | null {
  const trimmed = name.trim().slice(0, 80);
  if (!trimmed) return null;
  if (!store.profiles.some((profile) => profile.id === profileId)) return null;
  return {
    ...store,
    profiles: store.profiles.map((profile) => profile.id === profileId ? { ...profile, name: trimmed, updatedAt: now } : profile),
  };
}

export function deleteLearningProfile(store: LearningProfileStore, profileId: string): LearningProfileStore | null {
  if (store.profiles.length < 2) return null;
  const profiles = store.profiles.filter((profile) => profile.id !== profileId);
  if (profiles.length === store.profiles.length) return null;
  const activeProfileId = store.activeProfileId === profileId ? profiles[0]!.id : store.activeProfileId;
  return { ...store, activeProfileId, profiles };
}

export function replaceActiveMemory(store: LearningProfileStore, memory: CorrectionMemory, now = new Date().toISOString()): LearningProfileStore {
  const active = activeLearningProfile(store);
  return {
    ...store,
    profiles: store.profiles.map((profile) => profile.id === active.id ? { ...profile, memory, updatedAt: now } : profile),
  };
}
