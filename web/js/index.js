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
};
