// Run in a page on the Todo server. Each popup boots the real application.
(async () => {
    const source = "ewe.todo.pending.v1";
    const tabs = [];
    const old = {};
    const first = crypto.randomUUID(), late = crypto.randomUUID();
    function add(id, title) {
        old["add:" + id] = {change: {Add: {creation_id: id, title}}, retry: false};
        localStorage.setItem(source, JSON.stringify(old));
    }
    async function wait(read, explanation) {
        const deadline = performance.now() + 15000;
        while (performance.now() < deadline) {
            const value = read();
            if (value) return value;
            await new Promise(resolve => setTimeout(resolve, 25));
        }
        throw Error(explanation + ": " + tabs.map(tab => tab.document.body?.innerText).join(" / "));
    }
    async function blank() {
        const tab = window.open("/theme.css");
        if (!tab) throw Error("Cannot open a Todo migration test tab");
        tabs.push(tab);
        await wait(() => tab.document.readyState === "complete" && tab.location.pathname === "/theme.css", "Opening test tab");
        return tab;
    }
    function boot(tab) {
        const doc = tab.document;
        doc.body.innerHTML = '<main id="page"></main><output id="notice"></output>';
        const script = doc.createElement("script");
        script.type = "module";
        script.src = "/app.js";
        doc.body.append(script);
    }
    async function entries() {
        const name = "ewe.kv.todo:" + location.origin;
        if (!(await indexedDB.databases()).some(database => database.name === name)) return [];
        return new Promise((resolve, reject) => {
            const open = indexedDB.open(name);
            open.onerror = () => reject(open.error);
            open.onsuccess = () => {
                const db = open.result;
                const tx = db.transaction("records");
                const keys = tx.objectStore("records").getAllKeys();
                tx.oncomplete = () => { db.close(); resolve(keys.result); };
                tx.onabort = () => { db.close(); reject(tx.error); };
            };
        });
    }
    const button = (tab, id) => tab.document.querySelector('button[value="add:' + id + '"]');
    try {
        add(first, "Migrated Todo");
        old["add:wrong"] = old["add:" + first];
        localStorage.setItem(source, JSON.stringify(old));
        const left = await blank();
        boot(left);
        await wait(() => left.document.querySelector("#notice")?.value.includes("differs"), "Invalid legacy entry accepted");
        if (left.todoReady || (await entries()).some(key => key.endsWith(first))) throw Error("Invalid import wrote records");

        delete old["add:wrong"];
        localStorage.setItem(source, JSON.stringify(old));
        const aborted = await blank();
        const put = aborted.IDBObjectStore.prototype.put;
        aborted.IDBObjectStore.prototype.put = function(value, key) {
            const request = put.call(this, value, key);
            if (key.startsWith("imported/")) this.transaction.abort();
            return request;
        };
        boot(aborted);
        await wait(() => aborted.document.querySelector("#notice")?.value.includes("AbortError"), "Missing aborted migration error");
        if (aborted.todoReady || (await entries()).some(key => key.endsWith(first))) throw Error("Partial migration");

        const right = await blank();
        left.location.replace("/");
        right.location.replace("/");
        await wait(() => button(left, first) && button(right, first), "Concurrent recovery lost an operation");
        button(left, first).click();
        await wait(() => left.document.querySelector('button[value="' + first + '"]'), "Missing confirmed original identifier");
        add(late, "Late old tab");
        right.location.reload();
        await wait(() => button(right, late), "Missing late write");
        if (button(right, first)) throw Error("Confirmed operation restored");
        const keys = await entries();
        for (const id of [first, late]) {
            if (!keys.includes("imported/v1/add:" + id)) throw Error("Missing receipt");
        }
        if (keys.includes("pending/add:" + first) || !keys.includes("pending/add:" + late)) throw Error("Incorrect pending records after import");
        if (localStorage.getItem(source) !== JSON.stringify(old)) throw Error("Source map changed");
    } finally {
        for (const tab of tabs) tab.close();
        localStorage.removeItem(source);
    }
})().then(() => "ok", error => String(error))
