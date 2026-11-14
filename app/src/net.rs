//! WebSocket client used by the multiplayer lobby and matches.
//! Native: a background thread speaks WebSocket over TCP (ws:// only).
//! Browser: the page's WebSocket through the `net.js` plugin (ws:// or wss://).

#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NetStatus {
    Idle,
    Connecting,
    Open,
    Closed,
}

// ------------------------------------------------------------------ native
#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::NetStatus;
    use crate::wsproto::*;
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
    use std::time::Duration;

    enum Ev {
        Open,
        Msg(String),
        Closed(String),
    }

    pub struct Net {
        rx: Option<Receiver<Ev>>,
        tx: Option<Sender<String>>,
        pub status: NetStatus,
        pub error: String,
    }

    fn parse_url(url: &str) -> Option<(String, u16, String)> {
        let rest = url.strip_prefix("ws://")?;
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], rest[i..].to_string()),
            None => (rest, "/ws".to_string()),
        };
        let (host, port) = match hostport.rfind(':') {
            Some(i) => (hostport[..i].to_string(), hostport[i + 1..].parse().ok()?),
            None => (hostport.to_string(), 80),
        };
        Some((host, port, path))
    }

    fn run(url: String, ev: Sender<Ev>, out: Receiver<String>) {
        let fail = |e: &str| {
            let _ = ev.send(Ev::Closed(e.to_string()));
        };
        let (host, port, path) = match parse_url(&url) {
            Some(p) => p,
            None => return fail("SERVER ADDRESS MUST START WITH ws://"),
        };
        let addr = match (host.as_str(), port).to_socket_addrs().ok().and_then(|mut a| a.next()) {
            Some(a) => a,
            None => return fail("CAN'T FIND THE SERVER"),
        };
        let mut s = match TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
            Ok(s) => s,
            Err(_) => return fail("CAN'T REACH THE SERVER"),
        };
        let _ = s.set_nodelay(true);
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: cmV0YWxyZXRhbHJldGFsIQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            path, host, port
        );
        if s.write_all(req.as_bytes()).is_err() {
            return fail("CONNECTION FAILED");
        }
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut head = Vec::new();
        let mut b = [0u8; 4096];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match s.read(&mut b) {
                Ok(0) | Err(_) => return fail("SERVER DID NOT ANSWER"),
                Ok(n) => head.extend_from_slice(&b[..n]),
            }
        }
        let end = head.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        if !String::from_utf8_lossy(&head[..end]).starts_with("HTTP/1.1 101") {
            return fail("NOT A RETAL SERVER");
        }
        let mut fr = FrameReader::default();
        fr.push(&head[end..]);
        let _ = ev.send(Ev::Open);
        let _ = s.set_read_timeout(None);
        let mut w = match s.try_clone() {
            Ok(w) => w,
            Err(_) => return fail("CONNECTION FAILED"),
        };
        std::thread::spawn(move || {
            let mut k: u32 = 0x9e3779b9;
            for m in out {
                k ^= k << 13;
                k ^= k >> 17;
                k ^= k << 5;
                if w.write_all(&encode_frame(OP_TEXT, m.as_bytes(), Some(k.to_le_bytes()))).is_err() {
                    break;
                }
            }
            let _ = w.write_all(&encode_frame(OP_CLOSE, &[], Some([1, 2, 3, 4])));
            let _ = w.shutdown(std::net::Shutdown::Both);
        });
        loop {
            while let Some(f) = fr.next() {
                match f {
                    Ok((OP_TEXT, p)) => {
                        if ev.send(Ev::Msg(String::from_utf8_lossy(&p).to_string())).is_err() {
                            return;
                        }
                    }
                    Ok((OP_CLOSE, _)) | Err(()) => return fail("DISCONNECTED"),
                    _ => {}
                }
            }
            match s.read(&mut b) {
                Ok(0) | Err(_) => return fail("DISCONNECTED"),
                Ok(n) => fr.push(&b[..n]),
            }
        }
    }

    impl Net {
        pub fn new() -> Self {
            Net { rx: None, tx: None, status: NetStatus::Idle, error: String::new() }
        }
        pub fn connect(&mut self, url: &str) {
            self.close();
            let (etx, erx) = channel();
            let (otx, orx) = channel();
            let u = url.to_string();
            std::thread::spawn(move || run(u, etx, orx));
            self.rx = Some(erx);
            self.tx = Some(otx);
            self.status = NetStatus::Connecting;
            self.error.clear();
        }
        pub fn send(&mut self, msg: &str) {
            if self.status == NetStatus::Open {
                if let Some(t) = &self.tx {
                    let _ = t.send(msg.to_string());
                }
            }
        }
        pub fn poll(&mut self) -> Vec<String> {
            let mut out = Vec::new();
            if let Some(rx) = &self.rx {
                loop {
                    match rx.try_recv() {
                        Ok(Ev::Open) => self.status = NetStatus::Open,
                        Ok(Ev::Msg(m)) => out.push(m),
                        Ok(Ev::Closed(e)) => {
                            self.status = NetStatus::Closed;
                            self.error = e;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            if self.status != NetStatus::Closed {
                                self.status = NetStatus::Closed;
                            }
                            break;
                        }
                    }
                }
            }
            out
        }
        pub fn close(&mut self) {
            self.tx = None; // writer thread sends a close frame and shuts the socket
            self.rx = None;
            self.status = NetStatus::Idle;
        }
    }

    pub fn default_server() -> String {
        format!("ws://127.0.0.1:{}/ws", crate::server::DEFAULT_PORT)
    }

    pub fn prompt_text(_label: &str, _current: &str) -> Option<Option<String>> {
        None // native uses the in-game text box
    }
}

// ------------------------------------------------------------------ browser
#[cfg(target_arch = "wasm32")]
mod imp {
    use super::NetStatus;

    #[link(wasm_import_module = "env")]
    extern "C" {
        fn retal_ws_open(ptr: *const u8, len: u32);
        fn retal_ws_send(ptr: *const u8, len: u32);
        fn retal_ws_close();
        fn retal_ws_status() -> i32;
        fn retal_ws_next_len() -> i32;
        fn retal_ws_take(ptr: *mut u8, cap: u32) -> i32;
        fn retal_default_server(ptr: *mut u8, cap: u32) -> i32;
        fn retal_prompt(lptr: *const u8, llen: u32, dptr: *const u8, dlen: u32, optr: *mut u8, ocap: u32) -> i32;
    }

    #[no_mangle]
    pub extern "C" fn retal_net_crate_version() -> u32 {
        1
    }

    pub struct Net {
        pub status: NetStatus,
        pub error: String,
        started: bool,
    }

    impl Net {
        pub fn new() -> Self {
            Net { status: NetStatus::Idle, error: String::new(), started: false }
        }
        pub fn connect(&mut self, url: &str) {
            unsafe { retal_ws_open(url.as_ptr(), url.len() as u32) };
            self.status = NetStatus::Connecting;
            self.error.clear();
            self.started = true;
        }
        pub fn send(&mut self, msg: &str) {
            if self.status == NetStatus::Open {
                unsafe { retal_ws_send(msg.as_ptr(), msg.len() as u32) };
            }
        }
        pub fn poll(&mut self) -> Vec<String> {
            let mut out = Vec::new();
            if !self.started {
                return out;
            }
            loop {
                let n = unsafe { retal_ws_next_len() };
                if n < 0 {
                    break;
                }
                let mut buf = vec![0u8; n as usize];
                let got = unsafe { retal_ws_take(buf.as_mut_ptr(), n as u32) };
                if got >= 0 {
                    buf.truncate(got as usize);
                    out.push(String::from_utf8_lossy(&buf).to_string());
                }
            }
            let st = unsafe { retal_ws_status() };
            self.status = match st {
                1 => NetStatus::Connecting,
                2 => NetStatus::Open,
                3 => {
                    if self.status != NetStatus::Closed {
                        self.error = if self.status == NetStatus::Connecting { "CAN'T REACH THE SERVER".into() } else { "DISCONNECTED".into() };
                    }
                    NetStatus::Closed
                }
                _ => NetStatus::Idle,
            };
            out
        }
        pub fn close(&mut self) {
            unsafe { retal_ws_close() };
            self.status = NetStatus::Idle;
            self.started = false;
        }
    }

    pub fn default_server() -> String {
        let mut buf = vec![0u8; 512];
        let n = unsafe { retal_default_server(buf.as_mut_ptr(), 512) };
        buf.truncate(n.max(0) as usize);
        String::from_utf8_lossy(&buf).to_string()
    }

    /// Browser text entry via the page's prompt box (brings up the phone keyboard).
    /// Some(Some(text)) = entered, Some(None) = cancelled.
    pub fn prompt_text(label: &str, current: &str) -> Option<Option<String>> {
        let mut buf = vec![0u8; 256];
        let n = unsafe { retal_prompt(label.as_ptr(), label.len() as u32, current.as_ptr(), current.len() as u32, buf.as_mut_ptr(), 256) };
        if n < 0 {
            return Some(None);
        }
        buf.truncate(n as usize);
        Some(Some(String::from_utf8_lossy(&buf).to_string()))
    }
}

pub use imp::*;
