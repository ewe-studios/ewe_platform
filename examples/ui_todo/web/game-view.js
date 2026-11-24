// The todo mount keeps its loader and subscription while the game is visible.
export function installMathalley(frames) {
    const page = document.querySelector("#page");
    let host, active;
    function load() {
        host ??= import("./mathalley/ui_todo.js").then(async module => {
            await module.default();
            return { sound: new module.MathalleyHost(), graphics: new module.BrowserGraphics() };
        }).catch(error => { host = undefined; throw error; });
        return host;
    }
    function open() {
        if (active) return;
        const root = document.createElement("section");
        root.id = "mathalley";
        root.setAttribute("aria-label", "Mathalley");
        root.setAttribute("aria-busy", "true");
        const back = document.createElement("button");
        back.textContent = "Back to Todos";
        const canvas = document.createElement("canvas");
        canvas.setAttribute("aria-label", "Mathalley game");
        const status = document.createElement("span");
        status.setAttribute("role", "status");
        const focus = document.activeElement;
        root.append(back, canvas, status);
        document.body.append(root);
        const unregister = frames.registerView("mathalley", root);
        page.hidden = true;
        back.focus();
        let closed = false, frame = 0, surface, game, audio, nextAudio = 0;
        const playing = new Set();
        const listeners = new AbortController();
        const options = { signal: listeners.signal };
        function fail(error) {
            if (!closed) {
                cancelAnimationFrame(frame);
                status.textContent = 'Cannot play Mathalley. ' + (error.message ?? error);
                canvas.hidden = true;
            }
        }
        function stopNotes() {
            for (const source of playing) { source.stop(); source.disconnect(); }
            playing.clear();
            nextAudio = 0;
        }
        function scheduleAudio() {
            if (audio?.state !== "running") return;
            // Fill two hardware output intervals and one display interval.
            const duration = Math.max(audio.baseLatency || 0, 1/60);
            const length = Math.ceil(audio.sampleRate * duration);
            nextAudio = Math.max(audio.currentTime,nextAudio);
            while (nextAudio < audio.currentTime + duration * 3) {
                const buffer = audio.createBuffer(1,length,audio.sampleRate);
                buffer.copyToChannel(game.samples(nextAudio,audio.sampleRate,length),0);
                const source = audio.createBufferSource();
                source.buffer = buffer;
                source.connect(audio.destination);
                playing.add(source);
                source.onended = () => { playing.delete(source); source.disconnect(); };
                source.start(nextAudio);
                nextAudio += buffer.duration;
            }
        }
        function soundError(error) {
            status.textContent = "Sound unavailable. " + (error.message ?? error);
        }
        async function draw(time) {
            const ratio = devicePixelRatio;
            const width = Math.round(canvas.clientWidth * ratio);
            const height = Math.round(canvas.clientHeight * ratio);
            if (canvas.width !== width || canvas.height !== height) {
                canvas.width = width;
                canvas.height = height;
                await surface.resize(width,height);
            }
            if (closed || document.hidden) return;
            game.tick(time);
            const label=game.screen_name();
            if (canvas.getAttribute("aria-label") !== label) canvas.setAttribute("aria-label",label);
            try { scheduleAudio(); } catch (error) { soundError(error); }
            if (width && height && await game.draw(surface)) {
                root.setAttribute("aria-busy","false");
            }
        }
        let drawing = Promise.resolve();
        function schedule() {
            cancelAnimationFrame(frame);
            if (!closed && !document.hidden && !canvas.hidden)
                frame = requestAnimationFrame(time => {
                    drawing = drawing.then(() => draw(time)).then(schedule).catch(fail);
                });
        }
        async function start() {
            const {sound, graphics} = await load();
            if (closed) return;
            surface = await graphics.attach("mathalley", canvas);
            if (closed) return;
            game = sound.open(crypto.getRandomValues(new BigUint64Array(1))[0]);
            canvas.addEventListener("pointerdown", event => {
                const box = canvas.getBoundingClientRect();
                game.tap(event.clientX-box.left,event.clientY-box.top,box.width,box.height);
                try {
                    audio ??= new AudioContext();
                    audio.resume().catch(soundError);
                } catch (error) { soundError(error); }
            }, options);
            document.addEventListener("visibilitychange", () => {
                game.active(!document.hidden);
                if (audio) {
                    if (document.hidden) stopNotes();
                    (document.hidden ? audio.suspend() : audio.resume()).catch(soundError);
                }
                drawing.then(schedule);
            }, options);
            schedule();
        }
        const started = start().catch(fail);
        async function close() {
            if (closed) return;
            closed = true;
            cancelAnimationFrame(frame);
            listeners.abort();
            page.hidden = false;
            unregister();
            root.remove();
            focus?.focus();
            try {
                await started;
                await drawing;
                try { stopNotes(); game?.close(); if (audio) await audio.close(); }
                finally { if (surface) await surface.remove(); }
            } finally {
                game?.free();
                surface?.free();
                active = undefined;
            }
        }
        active = { close };
        back.addEventListener("click", () => close().catch(console.error), options);
    }
    return { open };
}
