async function lib() {
}

window.onload = async function() {
    let lib = await import("../../pkg");
    let room = new lib.RoomHandle();
    await room.test();
    console.log("Yay");
};
