window.onload = async function() {
    let lib = await import("../../pkg");
    let room = new lib.RoomHandle();
    await room.mute(true, true);
    console.log("Yay");
};
