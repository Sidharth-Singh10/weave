/**
 * Minimal IndexedDB promise wrapper powering Weave's binary-file tier.
 * The tier is currently a stub — no assets are stored yet — but the API
 * mirrors Excalidraw's `filesStore` (idb-keyval) so `FileStore` can grow
 * real image/library persistence without reshaping callers.
 *
 * Client-only: never import from a server-rendered module scope.
 */

export interface IDBStore {
  get<T>(key: IDBValidKey): Promise<T | undefined>;
  set(key: IDBValidKey, value: unknown): Promise<void>;
  getMany<T>(keys: IDBValidKey[]): Promise<T[]>;
  entries<T>(): Promise<[IDBValidKey, T][]>;
  del(key: IDBValidKey): Promise<void>;
  clear(): Promise<void>;
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function transactionDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

function store(db: IDBDatabase, name: string, mode: IDBTransactionMode) {
  const tx = db.transaction(name, mode);
  return { tx, store: tx.objectStore(name) };
}

export function createIDBStore(dbName: string, storeName: string): IDBStore {
  const open = () =>
    new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open(dbName, 1);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains(storeName)) {
          db.createObjectStore(storeName);
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });

  let dbPromise: Promise<IDBDatabase> | null = null;
  const db = () => (dbPromise ??= open());

  return {
    async get<T>(key: IDBValidKey): Promise<T | undefined> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readonly");
      const result = requestToPromise(s.get(key) as IDBRequest<T>);
      await transactionDone(tx);
      return result;
    },

    async set(key: IDBValidKey, value: unknown): Promise<void> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readwrite");
      s.put(value, key);
      await transactionDone(tx);
    },

    async getMany<T>(keys: IDBValidKey[]): Promise<T[]> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readonly");
      const results = keys.map(
        (k) => requestToPromise(s.get(k) as IDBRequest<T>)
      );
      await transactionDone(tx);
      return Promise.all(results);
    },

    async entries<T>(): Promise<[IDBValidKey, T][]> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readonly");
      const cursor = s.openCursor();
      const out: [IDBValidKey, T][] = [];
      cursor.onsuccess = () => {
        const cur = cursor.result;
        if (cur) {
          out.push([cur.key, cur.value as T]);
          cur.continue();
        }
      };
      await transactionDone(tx);
      return out;
    },

    async del(key: IDBValidKey): Promise<void> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readwrite");
      s.delete(key);
      await transactionDone(tx);
    },

    async clear(): Promise<void> {
      const d = await db();
      const { store: s, tx } = store(d, storeName, "readwrite");
      s.clear();
      await transactionDone(tx);
    },
  };
}
