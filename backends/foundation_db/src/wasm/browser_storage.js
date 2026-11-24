// Keep one application namespace in each database. Close the connection after each operation.
async function openDatabase(namespace) {
    const request = indexedDB.open("ewe.kv." + namespace, 1);
    return new Promise((resolve, reject) => {
        request.onupgradeneeded = () => request.result.createObjectStore("records");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
    });
}

async function indexedOperation(namespace, operation, key, value, after, limit) {
    const database = await openDatabase(namespace);
    try {
        return await new Promise((resolve, reject) => {
            const writes = operation === "set" || operation === "delete";
            const tx = database.transaction("records", writes ? "readwrite" : "readonly");
            const records = tx.objectStore("records");
            let result = null;
            tx.oncomplete = () => resolve(result);
            tx.onabort = () => reject(tx.error ?? new DOMException("Storage transaction aborted", "AbortError"));
            if (operation === "list") {
                const page = [];
                result = page;
                const cursor = records.openKeyCursor(IDBKeyRange.lowerBound(after ?? key, after != null));
                cursor.onsuccess = () => {
                    const entry = cursor.result;
                    if (!entry || !entry.key.startsWith(key)) return;
                    page.push(entry.key);
                    if (page.length < limit) entry.continue();
                };
            } else if (operation === "get") {
                const request = records.get(key);
                request.onsuccess = () => { result = request.result ?? null; };
            } else if (operation === "exists") {
                const request = records.count(key);
                request.onsuccess = () => { result = request.result !== 0; };
            } else if (operation === "set") records.put(value, key);
            else if (operation === "delete") records.delete(key);
            else throw new TypeError("Unknown storage operation");
        });
    } finally {
        database.close();
    }
}

function localOperation(namespace, operation, key, value, after, limit) {
    const storage = globalThis.localStorage;
    const scope = "ewe.kv." + JSON.stringify(namespace) + ":";
    switch (operation) {
        case "get": return storage.getItem(scope + key);
        case "exists": return storage.getItem(scope + key) !== null;
        case "set": storage.setItem(scope + key, value); return null;
        case "delete": storage.removeItem(scope + key); return null;
        case "list": {
            // Web Storage does not provide sorted iteration. Keep the next page in key order.
            const page = [];
            for (let i = 0; i < storage.length; i++) {
                const entry = storage.key(i);
                if (!entry?.startsWith(scope + key)) continue;
                const name = entry.slice(scope.length);
                if (after != null && name <= after) continue;
                if (page.length === limit && name >= page[limit - 1]) continue;
                const index = page.findIndex(item => item > name);
                page.splice(index < 0 ? page.length : index, 0, name);
                if (page.length > limit) page.pop();
            }
            return page;
        }
        default: throw new TypeError("Unknown storage operation");
    }
}

export async function browserOperation(indexed, namespace, operation, key, value, after, limit) {
    const run = indexed ? indexedOperation : localOperation;
    return run(namespace, operation, key, value, after, limit);
}
