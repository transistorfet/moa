 
import * as Emulator from './moa-genesis.js';

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
    reader.readAsArrayBuffer(file_input.files[0])
});

function initialize_emulator() {
    let host = Emulator.new_host();
    let system = Emulator.load_system(host, Emulator.get_load_system_fn());

    let last_update = performance.now();
    setTimeout(function refreshFrame() {
        let current = performance.now();
        let diff = Math.min(current - last_update, 100);
        let remaining = Math.max((16 * Emulator.get_speed()) - diff, 0);
        //console.log(diff, remaining);
        last_update = current;

        Emulator.run_system_for(system, diff * 1_000_000);
        if (Emulator.is_running()) {
                setTimeout(refreshFrame, remaining);
        }
    }, 0);
    /*
    setTimeout(function refreshFrame() {
        let run_time = run_system_for(system, 66_000_000);
        setTimeout(refreshFrame, 66 - run_time);
    }, 0);
    */

    Emulator.host_run_loop(host);
}

document.getElementById("reset").addEventListener("click", () => {
    Emulator.request_stop();
    //start();
});

document.getElementById("power").addEventListener("click", () => {
    if (Emulator.is_running())
        Emulator.request_stop();
    else
        initialize_emulator();
});

document.getElementById("speed").addEventListener("change", (e) => {
    Emulator.set_speed(e.target.value);
});

var file_input = document.getElementById("rom-file");
var frame_rate_el = document.getElementById("frame-rate");
var frame_rate = setInterval(function () {
    frame_rate_el.value = Emulator.get_frames_since();
}, 1000);

window.addEventListener("load", () => {
    Emulator.default();
});
