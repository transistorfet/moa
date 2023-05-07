
import * as Emulator from './moa-genesis.js';

window.addEventListener("load", () => {
    Emulator.default();
});

function initialize_emulator() {
    const host = Emulator.new_host();
    const system = Emulator.load_system(host, Emulator.get_load_system_fn());

    //Emulator.start_system(system);
    let last_update = performance.now();
    setTimeout(function refreshFrame() {
        // Calculate the time difference since the last update cycle
        const current = performance.now();
        const diff = current - last_update;
        last_update = current;

        // Run the system for the difference, and get the realtime runtime in millis
        const runtime = Emulator.run_system_for(system, diff * 1_000_000);

        if (Emulator.is_running()) {
            // Calculate the timeout needed to fill the time that was *not* taken by the sim
            const remaining = Math.max(diff - runtime - (diff * 0.1), 1);
            setTimeout(refreshFrame, remaining);
        }
    }, 0);

    const controllers = Emulator.get_controllers(host);
    function button_event(e) {
        let state;
        if (e.type == 'mousedown' || e.type == 'touchstart') {
            state = true;
        } else {
            state = false;
        }
        Emulator.button_press(controllers, e.target.name, state);
    }

    document.getElementById("controller").querySelectorAll('button').forEach(function (button) {
        button.addEventListener('mousedown', button_event);
        button.addEventListener('mouseup', button_event);
        button.addEventListener('touchstart', button_event);
        button.addEventListener('touchend', button_event);
    });

    Emulator.host_run_loop(host);
}

// Update the frame rate display
const frame_rate_el = document.getElementById("frame-rate");
const frame_rate = setInterval(function () {
    frame_rate_el.value = Emulator.get_frames_since();
}, 1000);

// Load a new ROM file
const reader = new FileReader();
reader.onloadend = function (e) {
    let data = new Uint8Array(reader.result);
    // If the SMD file magic number is present, then convert it before loading
    if (data[8] == 0xAA && data[9] == 0xBB)
        data = Emulator.smd_to_bin(data);
    Emulator.set_rom_data(data);
};

const file_input = document.getElementById("rom-file");
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

let mute_state = false;
const mute = document.getElementById("mute");
mute.addEventListener("click", () => {
    mute_state = !mute_state;
    if (mute_state) {
        mute.value = "\uD83D\uDD07";
    } else {
        mute.value = "\uD83D\uDD08";
    }
    Emulator.set_mute(mute_state);
});

