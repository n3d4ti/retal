// Tilt (gyroscope) bridge for the browser build.
// Reads the phone's orientation, turns it into a steering roll/pitch relative to the screen
// (works in landscape and portrait), and hands it to the game through miniquad's plugin system.
"use strict";

var retalTilt = {
    ok: 0,          // 1 once real orientation data has arrived
    roll: 0.0,      // radians, + = right edge of the screen tilted down
    pitch: 0.0,     // radians, + = top edge tilted toward the player (pull up)
    isTouch: ("ontouchstart" in window) || (navigator.maxTouchPoints > 0) ? 1 : 0,
    listening: false
};

function retalScreenAngle() {
    if (screen.orientation && typeof screen.orientation.angle === "number") {
        return screen.orientation.angle;
    }
    if (typeof window.orientation === "number") {
        return window.orientation;
    }
    return 0;
}

function retalOnOrientation(e) {
    if (!retalTilt.isTouch || e.beta === null || e.gamma === null || e.beta === undefined) {
        return;
    }
    var b = e.beta * Math.PI / 180.0;
    var g = e.gamma * Math.PI / 180.0;
    // "World up" expressed in device axes (x right, y toward top, z out of the screen).
    var ux = -Math.cos(b) * Math.sin(g);
    var uy = Math.sin(b);
    var uz = Math.cos(b) * Math.cos(g);
    // Rotate into screen axes using the current screen rotation.
    var th = retalScreenAngle() * Math.PI / 180.0;
    var sx = ux * Math.cos(th) - uy * Math.sin(th);
    var sy = ux * Math.sin(th) + uy * Math.cos(th);
    var sz = uz;
    // Steering: how far the screen's right edge dips.
    retalTilt.roll = Math.asin(Math.max(-1, Math.min(1, -sx)));
    // Pitch: rocking the top edge toward you (like pulling a yoke) is positive = climb.
    retalTilt.pitch = -Math.atan2(sy, sz);
    retalTilt.ok = 1;
}

function retalStartTilt() {
    if (retalTilt.listening) {
        return;
    }
    retalTilt.listening = true;
    window.addEventListener("deviceorientation", retalOnOrientation, true);
}

// iOS 13+ needs an explicit permission request from a user gesture.
function retalRequestTilt() {
    try {
        if (typeof DeviceOrientationEvent !== "undefined" &&
            typeof DeviceOrientationEvent.requestPermission === "function") {
            return DeviceOrientationEvent.requestPermission().then(function (state) {
                if (state === "granted") {
                    retalStartTilt();
                }
                return state;
            }).catch(function () { return "denied"; });
        }
    } catch (err) { /* ignore */ }
    retalStartTilt();
    return Promise.resolve("granted");
}

miniquad_add_plugin({
    name: "retal_tilt",
    version: 1,
    register_plugin: function (importObject) {
        importObject.env.retal_tilt_ok = function () { return retalTilt.ok; };
        importObject.env.retal_tilt_roll = function () { return retalTilt.roll; };
        importObject.env.retal_tilt_pitch = function () { return retalTilt.pitch; };
        importObject.env.retal_is_touch = function () { return retalTilt.isTouch; };
    },
    on_init: function () {
        // Android/desktop browsers deliver orientation without asking.
        if (typeof DeviceOrientationEvent === "undefined" ||
            typeof DeviceOrientationEvent.requestPermission !== "function") {
            retalStartTilt();
        }
    }
});
