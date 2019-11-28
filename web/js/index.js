window.onload = async function() {
    let lib = await import("../../pkg");
    let room = new lib.RoomHandle();

    document.getElementById('mute-video').addEventListener('click', async () => {
        await room.mute(false, true);
        console.log("Video muted!");
    });
    document.getElementById('mute-audio').addEventListener('click', async () => {
        await room.mute(true, false);
        console.log("Audio muted!");
    });
    document.getElementById('mute-all').addEventListener('click', async () => {
        await room.mute(true, true);
        console.log("All muted!");
    });
    document.getElementById('unmute-audio').addEventListener('click', async () => {
        await room.unmute(true, false);
        console.log("Audio unmuted!");
    });
    document.getElementById('unmute-video').addEventListener('click', async () => {
        await room.unmute(false, true);
        console.log("Video unmuted!");
    });
    document.getElementById('unmute-all').addEventListener('click', async () => {
        await room.unmute(true, true);
        console.log("All unmuted!");
    })
};
