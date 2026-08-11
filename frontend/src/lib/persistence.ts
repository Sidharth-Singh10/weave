"use client";

import { createIDBStore } from "./idb";
import { debounce } from "./debounce";
import { useGraphStore } from "./store";
import { usePersistenceStatus } from "./usePersistenceStatus";
import type {
  KnowledgeEdge,
  KnowledgeNode,
  PersistedScene,
  SemanticZoomLevel,
  SessionMeta,
  ViewType,
  XYPosition,
} from "./graph-types";

/** Match Excalidraw's SAVE_TO_LOCAL_STORAGE_TIMEOUT. */
export const SAVE_TIMEOUT_MS = 300;

const LS_SESSIONS = "weave:sessions";
const LS_ACTIVE = "weave:active-session";
const lsScene = (id: string) => `weave:session:${id}`;
const lsVersion = (id: string) => `weave:version:${id}`;

const ZOOM_LEVELS: SemanticZoomLevel[] = [
  "overview",
  "category",
  "entity",
  "detail",
];

const EMPTY_SCENE: PersistedScene = {
  version: 1,
  nodes: [],
  edges: [],
  positions: {},
  viewConfig: { type: "default", semanticZoom: "entity" },
  communityLabels: {},
  savedAt: Date.now(),
};

// ---------------------------------------------------------------------------
// localStorage primitives (all defensive — storage can throw in private mode)
// ---------------------------------------------------------------------------

function readLS(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch (error) {
    console.warn(`localStorage.getItem error: ${(error as Error).message}`);
    return null;
  }
}

type WriteResult = "ok" | "quota" | "error";

function writeLS(key: string, value: string): WriteResult {
  try {
    window.localStorage.setItem(key, value);
    return "ok";
  } catch (error) {
    console.warn(`localStorage.setItem error: ${(error as Error).message}`);
    return isQuotaExceededError(error) ? "quota" : "error";
  }
}

function removeLS(key: string): void {
  try {
    window.localStorage.removeItem(key);
  } catch (error) {
    console.warn(`localStorage.removeItem error: ${(error as Error).message}`);
  }
}

function isQuotaExceededError(error: unknown): boolean {
  return (
    error instanceof DOMException && error.name === "QuotaExceededError"
  );
}

// ---------------------------------------------------------------------------
// Session index
// ---------------------------------------------------------------------------

function generateId(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 9)}`;
}

function nextDefaultName(sessions: SessionMeta[]): string {
  return `Session ${sessions.length + 1}`;
}

export function listSessions(): SessionMeta[] {
  const raw = readLS(LS_SESSIONS);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((s) => s && typeof s.id === "string" && typeof s.name === "string")
      .map((s) => ({
        id: s.id,
        name: s.name,
        createdAt: typeof s.createdAt === "number" ? s.createdAt : 0,
        updatedAt: typeof s.updatedAt === "number" ? s.updatedAt : 0,
      }))
      .sort((a, b) => b.updatedAt - a.updatedAt);
  } catch {
    return [];
  }
}

function persistSessions(sessions: SessionMeta[]): void {
  writeLS(LS_SESSIONS, JSON.stringify(sessions));
}

function getActiveSessionId(): string | null {
  return readLS(LS_ACTIVE);
}

function setActiveSessionId(id: string | null): void {
  if (id === null) removeLS(LS_ACTIVE);
  else writeLS(LS_ACTIVE, id);
}

// ---------------------------------------------------------------------------
// Scene serialize / deserialize / migrate
// ---------------------------------------------------------------------------

function serializeSceneFromState(
  state: ReturnType<typeof useGraphStore.getState>
): PersistedScene {
  return {
    version: 1,
    nodes: state.knowledgeNodes,
    edges: state.knowledgeEdges,
    positions: state.positions,
    viewConfig: {
      type: state.viewConfig.type,
      semanticZoom: state.viewConfig.semanticZoom,
    },
    communityLabels: state.communityLabels,
    savedAt: Date.now(),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/** Schema-version migration seam. Future scene bumps add a case here; a scene
 * from an unknown/future version is discarded rather than corrupted. */
export function deserializeScene(raw: unknown): PersistedScene | null {
  if (!isRecord(raw)) return null;

  if (raw.version !== 1) return null;

  const nodes = Array.isArray(raw.nodes)
    ? (raw.nodes as KnowledgeNode[]).filter(
        (n) => n && typeof n.id === "string" && typeof n.label === "string"
      )
    : [];

  const edges = Array.isArray(raw.edges)
    ? (raw.edges as KnowledgeEdge[]).filter(
        (e) =>
          e &&
          typeof e.id === "string" &&
          typeof e.source === "string" &&
          typeof e.target === "string"
      )
    : [];

  const positions = isRecord(raw.positions)
    ? (raw.positions as Record<string, XYPosition>)
    : {};

  const communityLabels = isRecord(raw.communityLabels)
    ? (raw.communityLabels as Record<string, string>)
    : {};

  const vc = isRecord(raw.viewConfig) ? raw.viewConfig : {};
  const type: ViewType = vc.type === "topic" ? "topic" : "default";
  const semanticZoom: SemanticZoomLevel = ZOOM_LEVELS.includes(
    vc.semanticZoom as SemanticZoomLevel
  )
    ? (vc.semanticZoom as SemanticZoomLevel)
    : "entity";

  return {
    version: 1,
    nodes,
    edges,
    positions,
    viewConfig: { type, semanticZoom },
    communityLabels,
    savedAt: typeof raw.savedAt === "number" ? raw.savedAt : Date.now(),
  };
}

export function loadScene(id: string): PersistedScene | null {
  const raw = readLS(lsScene(id));
  if (!raw) return null;
  try {
    return deserializeScene(JSON.parse(raw));
  } catch {
    return null;
  }
}

function bumpSessionUpdatedAt(id: string): void {
  const sessions = listSessions().map((s) =>
    s.id === id ? { ...s, updatedAt: Date.now() } : s
  );
  persistSessions(sessions);
  useGraphStore.getState().setSessions(sessions);
}

function bumpVersion(id: string): void {
  writeLS(lsVersion(id), String(Date.now()));
}

/** Persist a scene blob for a session. Returns false when the write failed
 * (e.g. localStorage quota exceeded). */
function saveScene(id: string, scene: PersistedScene): boolean {
  const result = writeLS(lsScene(id), JSON.stringify(scene));
  if (result === "ok") {
    bumpSessionUpdatedAt(id);
    bumpVersion(id);
    usePersistenceStatus.getState().setQuotaExceeded(false);
    usePersistenceStatus.getState().setLastSavedAt(Date.now());
    return true;
  }
  if (result === "quota") {
    usePersistenceStatus.getState().setQuotaExceeded(true);
  }
  return false;
}

function removeScene(id: string): void {
  removeLS(lsScene(id));
  removeLS(lsVersion(id));
}

// ---------------------------------------------------------------------------
// Debounced save of the active session (Excalidraw `LocalData` analog)
// ---------------------------------------------------------------------------

let dirty = false;
let suppressSave = false;

const debouncedSave = debounce(() => {
  saveNow();
}, SAVE_TIMEOUT_MS);

export function isSavePaused(): boolean {
  return document.hidden;
}

function scheduleSave(): void {
  dirty = true;
  if (!isSavePaused()) {
    debouncedSave();
  }
}

/** Synchronously write the active session. Used by the debounce flush,
 * beforeunload, visibility transitions, and session switches. */
export function saveNow(): void {
  if (!dirty) return;
  dirty = false;
  const state = useGraphStore.getState();
  if (!state.activeSessionId) return;
  saveScene(state.activeSessionId, serializeSceneFromState(state));
}

function persistEmptyScene(): void {
  dirty = true;
  saveNow();
}

// ---------------------------------------------------------------------------
// Session operations
// ---------------------------------------------------------------------------

export function createSession(name?: string): SessionMeta {
  const sessions = listSessions();
  const meta: SessionMeta = {
    id: generateId(),
    name: name?.trim() || nextDefaultName(sessions),
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
  persistSessions([...sessions, meta]);
  useGraphStore.getState().setSessions([...sessions, meta]);
  switchSession(meta.id);
  return meta;
}

export function renameSession(id: string, name: string): void {
  const trimmed = name.trim();
  if (!trimmed) return;
  const sessions = listSessions().map((s) =>
    s.id === id ? { ...s, name: trimmed, updatedAt: Date.now() } : s
  );
  persistSessions(sessions);
  useGraphStore.getState().setSessions(sessions);
}

export function deleteSession(id: string): void {
  const sessions = listSessions().filter((s) => s.id !== id);
  const activeId = useGraphStore.getState().activeSessionId;

  if (id === activeId) {
    // Flush before removing so a pending save can't resurrect the blob.
    if (dirty) saveNow();
    debouncedSave.cancel();
    removeScene(id);

    if (sessions.length === 0) {
      const meta: SessionMeta = {
        id: generateId(),
        name: "Session 1",
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      sessions.push(meta);
      persistSessions(sessions);
      useGraphStore.getState().setSessions(sessions);
    } else {
      persistSessions(sessions);
      useGraphStore.getState().setSessions(sessions);
    }
    const nextId = sessions[0].id;
    setActiveSessionId(nextId);
    const scene = loadScene(nextId);
    suppressSave = true;
    useGraphStore.getState().hydrateSession(scene ?? EMPTY_SCENE);
    suppressSave = false;
    if (!scene) persistEmptyScene();
  } else {
    removeScene(id);
    persistSessions(sessions);
    useGraphStore.getState().setSessions(sessions);
  }
}

/** Flush pending changes of the current session, then load `nextId`'s saved
 * scene into the store. Never loses data on switch. */
export function switchSession(nextId: string): void {
  const state = useGraphStore.getState();
  if (nextId === state.activeSessionId) return;
  if (dirty) saveNow();
  debouncedSave.cancel();

  const scene = loadScene(nextId);
  useGraphStore.setState({ activeSessionId: nextId });
  setActiveSessionId(nextId);
  suppressSave = true;
  state.hydrateSession(scene ?? EMPTY_SCENE);
  suppressSave = false;
  if (!scene) persistEmptyScene();
}

/** Wipe the active session's scene, keeping the session itself. */
export function resetActiveSession(): void {
  const state = useGraphStore.getState();
  if (!state.activeSessionId) return;
  if (dirty) saveNow();
  debouncedSave.cancel();
  removeScene(state.activeSessionId);
  suppressSave = true;
  state.hydrateSession(EMPTY_SCENE);
  suppressSave = false;
  persistEmptyScene();
}

export function flushSave(): void {
  if (dirty) saveNow();
  debouncedSave.cancel();
}

// ---------------------------------------------------------------------------
// Cross-tab sync (Excalidraw `tabSync` analog)
// ---------------------------------------------------------------------------

function onStorage(event: StorageEvent): void {
  if (event.key == null) return;

  if (event.key === LS_SESSIONS) {
    useGraphStore.getState().setSessions(listSessions());
    return;
  }

  const activeId = useGraphStore.getState().activeSessionId;
  if (activeId && event.key === lsVersion(activeId) && event.newValue != null) {
    const scene = loadScene(activeId);
    if (scene) {
      suppressSave = true;
      useGraphStore.getState().hydrateSession(scene);
      suppressSave = false;
    }
  }
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

/** Subscribe the store to the debounced saver, hydrate the active session,
 * and register lifecycle listeners. Returns a cleanup function. */
export function initPersistence(): () => void {
  const state = useGraphStore.getState();

  let sessions = listSessions();
  if (sessions.length === 0) {
    const meta: SessionMeta = {
      id: generateId(),
      name: "Session 1",
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    persistSessions([meta]);
    sessions = [meta];
  }
  state.setSessions(sessions);

  let activeId = getActiveSessionId();
  if (!activeId || !sessions.some((s) => s.id === activeId)) {
    activeId = sessions[0].id;
    setActiveSessionId(activeId);
  }
  state.setActiveSessionId(activeId);

  const scene = loadScene(activeId);
  if (scene) {
    // Subscription isn't registered yet, so hydration can't trigger a save —
    // no suppress flag to set here (a leftover flag would swallow the next edit).
    state.hydrateSession(scene);
    const hasContent =
      scene.nodes.length > 0 ||
      scene.edges.length > 0 ||
      Object.keys(scene.positions).length > 0;
    if (hasContent) {
      const meta = sessions.find((s) => s.id === activeId);
      usePersistenceStatus.getState().setRestoredSession(meta?.name ?? null);
    }
  } else {
    persistEmptyScene();
  }

  const unsubscribe = useGraphStore.subscribe((next, prev) => {
    const persistable =
      next.knowledgeNodes !== prev.knowledgeNodes ||
      next.knowledgeEdges !== prev.knowledgeEdges ||
      next.positions !== prev.positions ||
      next.communityLabels !== prev.communityLabels ||
      next.viewConfig.type !== prev.viewConfig.type ||
      next.viewConfig.semanticZoom !== prev.viewConfig.semanticZoom;
    if (!persistable) return;
    if (suppressSave) {
      suppressSave = false;
      return;
    }
    scheduleSave();
  });

  const onBeforeUnload = () => {
    if (dirty) saveNow();
  };
  const onVisibilityChange = () => {
    if (!document.hidden && dirty) {
      debouncedSave.cancel();
      saveNow();
    }
  };
  const onStorageEvent = (event: StorageEvent) => onStorage(event);

  window.addEventListener("beforeunload", onBeforeUnload);
  document.addEventListener("visibilitychange", onVisibilityChange);
  window.addEventListener("storage", onStorageEvent);

  return () => {
    if (dirty) saveNow();
    unsubscribe();
    debouncedSave.cancel();
    window.removeEventListener("beforeunload", onBeforeUnload);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    window.removeEventListener("storage", onStorageEvent);
  };
}

// ---------------------------------------------------------------------------
// Binary-file tier (stub). Mirrors Excalidraw's FileManager/LocalFileManager
// so image/library persistence can land without reshaping callers.
// ---------------------------------------------------------------------------

interface StoredFileData {
  id: string;
  lastRetrieved?: number;
}

export class FileStore {
  private readonly store = createIDBStore("weave-files", "files-store");

  /** Write newly added binary files. Errors surface per file. */
  async saveFiles(files: Record<string, unknown>): Promise<void> {
    await Promise.all(
      Object.entries(files).map(([id, data]) => {
        const record =
          data !== null && typeof data === "object" ? (data as object) : {};
        return this.store.set(id, { ...record, id });
      })
    );
  }

  /** Load files by id and refresh their `lastRetrieved` timestamps. */
  async getFiles(ids: string[]): Promise<StoredFileData[]> {
    const loaded = await this.store.getMany<StoredFileData>(ids);
    const freshened = loaded.filter(Boolean).map((data) => ({
      ...data,
      lastRetrieved: Date.now(),
    }));
    await Promise.all(
      freshened.map((data) => this.store.set(data.id, data))
    );
    return freshened;
  }

  /** GC: drop files not referenced by the current scene and unused for a day
   * (Excalidraw's 24h obsolete-file rule). */
  async clearObsoleteFiles(opts: { currentFileIds: string[] }): Promise<void> {
    const current = new Set(opts.currentFileIds);
    const entries = await this.store.entries<StoredFileData>();
    const now = Date.now();
    await Promise.all(
      entries.map(([id, data]) => {
        const unused =
          !current.has(String(id)) &&
          (!data.lastRetrieved || now - data.lastRetrieved > 24 * 3600 * 1000);
        return unused ? this.store.del(id) : Promise.resolve();
      })
    );
  }

  clear(): Promise<void> {
    return this.store.clear();
  }
}

/** Shared instance for the file tier. */
export const fileStore = new FileStore();

// Re-export status helpers for the UI.
export { isQuotaExceededError };
