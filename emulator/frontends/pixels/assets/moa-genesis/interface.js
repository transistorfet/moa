
import * as Emulator from './moa-genesis.js';

function initialize_emulator() {
    let host = Emulator.new_host();
    let system = Emulator.load_system(host, Emulator.get_load_system_fn());

    //Emulator.start_system(system);
    let last_update = performance.now();
    setTimeout(function refreshFrame() {
        let current = performance.now();
        let diff = current - last_update;
        //let remaining = Math.max((16 * Emulator.get_speed()) - diff, 0);
        last_update = current;

        let runtime = Emulator.run_system_for(system, diff * 1_000_000);
        if (Emulator.is_running()) {
            let remaining = Math.max(diff - runtime - (diff * 0.1), 1);
            setTimeout(refreshFrame, remaining);
        }
    }, 0);

    Emulator.host_run_loop(host);
}

// Update the frame rate display
var frame_rate_el = document.getElementById("frame-rate");
var frame_rate = setInterval(function () {
    frame_rate_el.value = Emulator.get_frames_since();
}, 1000);

window.addEventListener("load", () => {
    Emulator.default();
});

// Load a new ROM file
var reader = new FileReader();
reader.onloadend = function (e) {
    var data = new Uint8Array(reader.result);
    // If the SMD file magic number is present, then convert it before loading
    if (data[8] == 0xAA && data[9] == 0xBB)
        data = Emulator.smd_to_bin(data);
    Emulator.set_rom_data(data);
};

var file_input = document.getElementById("rom-file");
file_input.addEventListener("change", e => {
    document.getElementById("video").focus();
    reader.readAsArrayBuffer(file_input.files[0])
});

document.getElementById("reset").addEventListener("click", () => {
    document.getElementById("video").focus();
    Emulator.request_stop();
});

document.getElementById("power").addEventListener("click", () => {
    document.getElementById("video").focus();
    if (Emulator.is_running())
        Emulator.request_stop();
    else
        initialize_emulator();
});

var mute_state = false;
var mute = document.getElementById("mute");
mute.addEventListener("click", () => {
    mute_state = !mute_state;
    if (mute_state) {
        mute.value = "\uD83D\uDD07";
    } else {
        mute.value = "\uD83D\uDD08";
    }
    Emulator.set_mute(mute_state);
});

function button_event(e) {
    var state;
    if (e.type == 'mousedown' || e.type == 'touchstart') {
        state = true;
    } else {
        state = false;
    }
    Emulator.button_press(e.target.name, state);
}

document.getElementById("controller").querySelectorAll('button').forEach(function (button) {
    button.addEventListener('mousedown', button_event);
    button.addEventListener('mouseup', button_event);
    button.addEventListener('touchstart', button_event);
    button.addEventListener('touchend', button_event);
});
