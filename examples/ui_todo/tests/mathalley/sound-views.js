// Render two independent sound sessions through the browser audio backend.
(async () => {
    const {MathalleyHost}=await import("/mathalley/ui_todo.js");
    const host=new MathalleyHost();
    const game=host.open(8n), other=host.open(8n);
    const rate=48000, length=4096;
    async function record(session,at) {
        const output=new OfflineAudioContext(1,length,rate);
        const buffer=output.createBuffer(1,length,rate);
        buffer.copyToChannel(session.samples(at,rate,length),0);
        const source=output.createBufferSource();
        source.buffer=buffer;
        source.connect(output.destination);
        source.start();
        return (await output.startRendering()).getChannelData(0);
    }
    try {
        game.tap(195,422,390,844); game.tick(0);
        const withCue=await record(game,0), baseline=await record(other,0);
        if (!withCue.some((v,i)=>Math.abs(v-baseline[i])>0.001)) throw Error("Missing game cue");
        game.tap(292,798,390,844);
        game.active(false); game.active(true);
        if ((await record(game,20)).some(v=>v!==0)) throw Error("Interruption changed mute");
        game.close();
        if (!(await record(other,20)).some(v=>Math.abs(v)>0.001)) throw Error("Second view lost sound");
        other.close();
        return true;
    } finally {game.free();other.free();host.free();}
})()
