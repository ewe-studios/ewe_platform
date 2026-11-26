// Each tab loads a separate instance of the standalone Rust consumer.
export async function verifyTabs() {
    const moduleUrl = new URL("foundation_db_browser_tests.js", location.href).href;
    const tabs = [];
    const page = new Blob([`<!doctype html><script type="module">
        try {
            window.consumer = await import(${JSON.stringify(moduleUrl)});
            await consumer.default();
            opener.postMessage({storageTest: "ready"}, location.origin);
        } catch (error) {
            opener.postMessage({storageTest: "error", detail: String(error)}, location.origin);
        }
    </script>`], {type: "text/html"});
    const pageUrl = URL.createObjectURL(page);
    function navigate(tab, start) {
        return new Promise((resolve, reject) => {
            function ready(event) {
                if (event.source !== tab || event.origin !== location.origin || !event.data?.storageTest) return;
                window.removeEventListener("message", ready);
                if (event.data.storageTest === "error") reject(new Error(event.data.detail));
                else resolve();
            }
            window.addEventListener("message", ready);
            start();
        });
    }
    async function expect(tab, indexed, rows) {
        const actual = await tab.consumer.entries(indexed, "tabs");
        if (actual !== JSON.stringify(rows)) throw new Error("Unexpected pending records: " + actual);
    }
    try {
        for (let i = 0; i < 2; i++) {
            const tab = window.open("about:blank");
            if (!tab) throw new Error("Cannot open a storage test tab");
            tabs.push(tab);
            await navigate(tab, () => tab.location.replace(pageUrl));
        }
        for (const indexed of [true, false]) {
            const [left, right] = tabs;
            await Promise.all([
                left.consumer.record(indexed, "tabs", "pending/a", "first"),
                right.consumer.record(indexed, "tabs", "pending/b", "second"),
            ]);
            await Promise.all(tabs.map(tab => navigate(tab, () => tab.location.reload())));
            for (const tab of tabs) await expect(tab, indexed, {"pending/a":"first", "pending/b":"second"});
            await left.consumer.record(indexed, "tabs", "pending/a", undefined);
            await expect(right, indexed, {"pending/b":"second"});
            await right.consumer.record(indexed, "tabs", "pending/b", undefined);
        }
        const tab = tabs[0];
        async function failure(indexed, name) {
            try {
                await tab.consumer.record(indexed, "tabs", "pending/error", "value");
            } catch (error) {
                if (String(error).startsWith(name + ":")) return;
                throw error;
            }
            throw new Error("Missing storage failure: " + name);
        }
        tab.Storage.prototype.setItem = () => {
            throw new DOMException("Quota reached", "QuotaExceededError");
        };
        await failure(false, "QuotaExceededError");
        Object.defineProperty(tab, "localStorage", {
            get() { throw new DOMException("Access denied", "SecurityError"); },
        });
        await failure(false, "SecurityError");
        const put = tab.IDBObjectStore.prototype.put;
        tab.IDBObjectStore.prototype.put = function (...args) {
            const request = put.apply(this, args);
            this.transaction.abort();
            return request;
        };
        await failure(true, "AbortError");
        await expect(tabs[1], true, {});
    } finally {
        for (const tab of tabs) tab.close();
        URL.revokeObjectURL(pageUrl);
    }
}
