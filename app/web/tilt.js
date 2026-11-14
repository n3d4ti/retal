// Tilt (gyroscope) bridge for the browser build.
// Reads the phone's orientation, turns it into a steering roll/pitch relative to the screen
// (works in landscape and portrait), smooths out sensor jitter, and hands it to the game
// through miniquad's plugin system.
"use strict";

var retalTilt = {
    ok: 0,          // 1 once real orientation data has arrived
    state: 0,       // 0 waiting, 1 working, 2 permission refused, 3 no sensor
    roll: 0.0,      // radians, + = right edge of the screen tilted down
    pitch: 0.0,     // radians, + = top edge rocked toward the player (climb)
    epoch: 0,       // bumps when the screen rotates, so the game re-levels
    isTouch: ("ontouchstart" in window) || (navigator.maxTouchPoints > 0) ? 1 : 0,
    listening: false,
    lastAngle: null,
    smooth: 0.35    // 0..1, share of each new reading (lower = smoother, slower)
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
    var angle = retalScreenAngle();
    if (retalTilt.lastAngle !== null && angle !== retalTilt.lastAngle) {
        retalTilt.epoch += 1;
        retalTilt.ok = 0; // restart smoothing from the new pose
    }
    retalTilt.lastAngle = angle;
    var th = angle * Math.PI / 180.0;
    var sx = ux * Math.cos(th) - uy * Math.sin(th);
    var sy = ux * Math.sin(th) + uy * Math.cos(th);
    var sz = uz;
    // Steering: how far the screen's right edge dips.
    var roll = Math.asin(Math.max(-1, Math.min(1, -sx)));
    // Pitch: rocking the top edge toward you (like pulling a yoke) is positive = climb.
    var pitch = -Math.atan2(sy, sz);
    if (!retalTilt.ok) {
        retalTilt.roll = roll;
        retalTilt.pitch = pitch;
    } else {
        // low-pass filter; unwrap pitch so it never jumps across +-180 degrees
        var dp = pitch - retalTilt.pitch;
        if (dp > Math.PI) { dp -= 2 * Math.PI; }
        if (dp < -Math.PI) { dp += 2 * Math.PI; }
        retalTilt.roll += (roll - retalTilt.roll) * retalTilt.smooth;
        retalTilt.pitch += dp * retalTilt.smooth;
    }
    retalTilt.ok = 1;
    retalTilt.state = 1;
}

function retalStartTilt() {
    if (retalTilt.listening) {
        return;
    }
    retalTilt.listening = true;
    if (typeof DeviceOrientationEvent === "undefined") {
        retalTilt.state = 3;
        return;
    }
    window.addEventListener("deviceorientation", retalOnOrientation, true);
    // no reading after a few seconds: treat as "no sensor" (the game shows drag-to-steer)
    setTimeout(function () {
        if (retalTilt.state === 0) { retalTilt.state = 3; }
    }, 4000);
}

// iOS 13+ needs an explicit permission request from a user gesture.
function retalRequestTilt() {
    try {
        if (typeof DeviceOrientationEvent !== "undefined" &&
            typeof DeviceOrientationEvent.requestPermission === "function") {
            return DeviceOrientationEvent.requestPermission().then(function (state) {
                if (state === "granted") {
                    retalStartTilt();
                } else {
                    retalTilt.state = 2;
                }
                return state;
            }).catch(function () { retalTilt.state = 2; return "denied"; });
        }
    } catch (err) { /* ignore */ }
    retalStartTilt();
    return Promise.resolve("granted");
}

// Keep the phone screen on while playing (tilt-only flying has few screen touches).
var retalWakeLock = null;
function retalKeepAwake() {
    try {
        if (navigator.wakeLock && navigator.wakeLock.request) {
            navigator.wakeLock.request("screen").then(function (l) { retalWakeLock = l; }).catch(function () { });
        }
    } catch (e) { /* not supported */ }
}
document.addEventListener("visibilitychange", function () {
    if (document.visibilityState === "visible" && retalWakeLock !== null) {
        retalKeepAwake();
    }
});

miniquad_add_plugin({
    name: "retal_tilt",
    version: 1,
    register_plugin: function (importObject) {
        importObject.env.retal_tilt_ok = function () { return retalTilt.ok; };
        importObject.env.retal_tilt_roll = function () { return retalTilt.roll; };
        importObject.env.retal_tilt_pitch = function () { return retalTilt.pitch; };
        importObject.env.retal_is_touch = function () { return retalTilt.isTouch; };
        importObject.env.retal_tilt_epoch = function () { return retalTilt.epoch; };
        importObject.env.retal_tilt_state = function () { return retalTilt.state; };
    },
    on_init: function () {
        // Android/desktop browsers deliver orientation without asking.
        if (typeof DeviceOrientationEvent === "undefined" ||
            typeof DeviceOrientationEvent.requestPermission !== "function") {
            retalStartTilt();
        }
    }
});
