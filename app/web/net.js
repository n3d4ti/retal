// Multiplayer bridge for the browser build: one WebSocket to the Retal server,
// plus a text prompt (brings up the phone keyboard) for names, codes and passwords.
"use strict";

var retalNet = { ws: null, status: 0, queue: [] };

function retalReadStr(ptr, len) {
    return new TextDecoder().decode(new Uint8Array(wasm_memory.buffer, ptr, len));
}

function retalWriteStr(str, ptr, cap) {
    var bytes = new TextEncoder().encode(str);
    var n = Math.min(bytes.length, cap);
    new Uint8Array(wasm_memory.buffer, ptr, n).set(bytes.subarray(0, n));
    return n;
}

miniquad_add_plugin({
    name: "retal_net",
    version: 1,
    register_plugin: function (importObject) {
        var env = importObject.env;
        env.retal_ws_open = function (ptr, len) {
            var url = retalReadStr(ptr, len);
            try { if (retalNet.ws) { retalNet.ws.close(); } } catch (e) { }
            retalNet.queue = [];
            retalNet.status = 1;
            try {
                var ws = new WebSocket(url);
                retalNet.ws = ws;
                ws.onopen = function () { if (retalNet.ws === ws) { retalNet.status = 2; } };
                ws.onmessage = function (ev) {
                    if (retalNet.ws === ws && typeof ev.data === "string") {
                        retalNet.queue.push(new TextEncoder().encode(ev.data));
                    }
                };
                ws.onclose = function () { if (retalNet.ws === ws) { retalNet.status = 3; } };
                ws.onerror = function () { if (retalNet.ws === ws) { retalNet.status = 3; } };
            } catch (e) {
                retalNet.status = 3;
            }
        };
        env.retal_ws_send = function (ptr, len) {
            if (retalNet.ws && retalNet.status === 2) {
                try { retalNet.ws.send(retalReadStr(ptr, len)); } catch (e) { }
            }
        };
        env.retal_ws_close = function () {
            try { if (retalNet.ws) { retalNet.ws.close(); } } catch (e) { }
            retalNet.ws = null;
            retalNet.status = 0;
            retalNet.queue = [];
        };
        env.retal_ws_status = function () { return retalNet.status; };
        env.retal_ws_next_len = function () {
            return retalNet.queue.length > 0 ? retalNet.queue[0].length : -1;
        };
        env.retal_ws_take = function (ptr, cap) {
            var b = retalNet.queue.shift();
            if (!b) { return -1; }
            var n = Math.min(b.length, cap);
            new Uint8Array(wasm_memory.buffer, ptr, n).set(b.subarray(0, n));
            return n;
        };
        env.retal_default_server = function (ptr, cap) {
            var proto = location.protocol === "https:" ? "wss://" : "ws://";
            var host = location.host || ("127.0.0.1:8787");
            return retalWriteStr(proto + host + "/ws", ptr, cap);
        };
        env.retal_prompt = function (lptr, llen, dptr, dlen, optr, ocap) {
            var label = retalReadStr(lptr, llen);
            var def = retalReadStr(dptr, dlen);
            var r = window.prompt(label, def);
            if (r === null || r === undefined) { return -1; }
            return retalWriteStr(r, optr, ocap);
        };
    }
});
