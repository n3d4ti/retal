//! Retal match server: serves the browser game (web/ folder) and runs the multiplayer lobby over WebSocket (/ws).
//!
//! Lobby protocol (text messages, fields split by '|'):
//!   client -> server: HELLO|name, LIST, CREATE|name|public|password|mode|diff, JOIN|code|password,
//!                     LEAVE, R|payload (to everyone else in the match), T|id|payload (to one player), PING
//!   server -> client: WELCOME|id, ROOMS|code~name~players~max~mode~locked~diff;...,
//!                     JOINED|code|my_id|host_id|mode|diff|name|public, PEER|id|name, PEERLEFT|id,
//!                     CLOSED|reason, ERR|reason, F|sender|payload, PONG

#![allow(dead_code)]

use crate::wsproto::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MAX_PLAYERS: usize = 8;
pub const DEFAULT_PORT: u16 = 8787;

struct Client {
    name: String,
    tx: Sender<Vec<u8>>,
    room: Option<String>,
}

struct Room {
    code: String,
    name: String,
    public: bool,
    password: String,
    host: u32,
    mode: String,
    diff: u8,
    members: Vec<u32>,
}

#[derive(Default)]
struct Lobby {
    next_id: u32,
    clients: HashMap<u32, Client>,
    rooms: HashMap<String, Room>,
    seed: u64,
}

impl Lobby {
    fn send(&self, id: u32, text: &str) {
        if let Some(c) = self.clients.get(&id) {
            let _ = c.tx.send(encode_frame(OP_TEXT, text.as_bytes(), None));
        }
    }

    fn code(&mut self) -> String {
        const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        loop {
            let mut s = String::new();
            for _ in 0..4 {
                self.seed ^= self.seed << 13;
                self.seed ^= self.seed >> 7;
                self.seed ^= self.seed << 17;
                s.push(A[(self.seed % A.len() as u64) as usize] as char);
            }
            if !self.rooms.contains_key(&s) {
                return s;
            }
        }
    }

    fn leave(&mut self, id: u32) {
        let code = match self.clients.get_mut(&id).and_then(|c| c.room.take()) {
            Some(c) => c,
            None => return,
        };
        let mut close = false;
        let mut others = Vec::new();
        if let Some(r) = self.rooms.get_mut(&code) {
            r.members.retain(|m| *m != id);
            others = r.members.clone();
            close = r.host == id || r.members.is_empty();
        }
        if close {
            self.rooms.remove(&code);
            for o in others {
                if let Some(c) = self.clients.get_mut(&o) {
                    c.room = None;
                }
                self.send(o, "CLOSED|HOST LEFT THE MATCH");
            }
        } else {
            for o in others {
                self.send(o, &format!("PEERLEFT|{}", id));
            }
        }
    }

    fn join(&mut self, id: u32, code: &str) {
        let (members, host, mode, diff, name, public) = {
            let r = self.rooms.get_mut(code).unwrap();
            r.members.push(id);
            (r.members.clone(), r.host, r.mode.clone(), r.diff, r.name.clone(), r.public)
        };
        if let Some(c) = self.clients.get_mut(&id) {
            c.room = Some(code.to_string());
        }
        self.send(id, &format!("JOINED|{}|{}|{}|{}|{}|{}|{}", code, id, host, mode, diff, name, if public { 1 } else { 0 }));
        let my_name = self.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        for m in members.iter().filter(|m| **m != id) {
            let n = self.clients.get(m).map(|c| c.name.clone()).unwrap_or_default();
            self.send(id, &format!("PEER|{}|{}", m, n));
            self.send(*m, &format!("PEER|{}|{}", id, my_name));
        }
    }

    fn handle(&mut self, id: u32, msg: &str) {
        let mut parts = msg.splitn(2, '|');
        let cmd = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        match cmd {
            "HELLO" => {
                let name = clean_field(rest, 16);
                let name = if name.is_empty() { format!("PILOT{}", id) } else { name };
                if let Some(c) = self.clients.get_mut(&id) {
                    c.name = name;
                }
                self.send(id, &format!("WELCOME|{}", id));
            }
            "LIST" => {
                let mut rooms: Vec<&Room> = self.rooms.values().collect();
                rooms.sort_by(|a, b| a.code.cmp(&b.code));
                let list: Vec<String> = rooms
                    .iter()
                    .map(|r| format!("{}~{}~{}~{}~{}~{}~{}", r.code, r.name, r.members.len(), MAX_PLAYERS, r.mode, if r.public { 0 } else { 1 }, r.diff))
                    .collect();
                self.send(id, &format!("ROOMS|{}", list.join(";")));
            }
            "CREATE" => {
                self.leave(id);
                let f: Vec<&str> = rest.split('|').collect();
                let name = clean_field(f.first().copied().unwrap_or(""), 20);
                let public = f.get(1).map(|v| *v == "1").unwrap_or(true);
                let password = f.get(2).copied().unwrap_or("").chars().take(32).collect::<String>();
                let mode = if f.get(3).copied() == Some("DOGFIGHT") { "DOGFIGHT" } else { "COOP" }.to_string();
                let diff = f.get(4).and_then(|v| v.parse::<u8>().ok()).unwrap_or(1).min(2);
                if !public && password.is_empty() {
                    self.send(id, "ERR|A PRIVATE MATCH NEEDS A PASSWORD");
                    return;
                }
                let code = self.code();
                let host_name = self.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
                let name = if name.is_empty() { format!("{}'S MATCH", host_name) } else { name };
                self.rooms.insert(code.clone(), Room { code: code.clone(), name, public, password, host: id, mode, diff, members: Vec::new() });
                self.join(id, &code);
            }
            "JOIN" => {
                let f: Vec<&str> = rest.split('|').collect();
                let code = f.first().copied().unwrap_or("").trim().to_uppercase();
                let pw = f.get(1).copied().unwrap_or("");
                let check = match self.rooms.get(&code) {
                    None => Err("NO MATCH WITH THAT CODE"),
                    Some(r) if r.members.len() >= MAX_PLAYERS => Err("MATCH IS FULL"),
                    Some(r) if !r.public && r.password != pw => Err("WRONG PASSWORD"),
                    Some(_) => Ok(()),
                };
                match check {
                    Err(e) => self.send(id, &format!("ERR|{}", e)),
                    Ok(()) => {
                        self.leave(id);
                        self.join(id, &code);
                    }
                }
            }
            "LEAVE" => self.leave(id),
            "R" => {
                if let Some(code) = self.clients.get(&id).and_then(|c| c.room.clone()) {
                    if let Some(r) = self.rooms.get(&code) {
                        let out = format!("F|{}|{}", id, rest);
                        for m in r.members.iter().filter(|m| **m != id) {
                            self.send(*m, &out);
                        }
                    }
                }
            }
            "T" => {
                let mut p = rest.splitn(2, '|');
                let to = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                let payload = p.next().unwrap_or("");
                let same_room = {
                    let a = self.clients.get(&id).and_then(|c| c.room.clone());
                    let b = self.clients.get(&to).and_then(|c| c.room.clone());
                    a.is_some() && a == b
                };
                if same_room {
                    self.send(to, &format!("F|{}|{}", id, payload));
                }
            }
            "PING" => self.send(id, "PONG"),
            _ => self.send(id, "ERR|UNKNOWN COMMAND"),
        }
    }
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" => "application/javascript",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "json" => "application/json",
        "css" => "text/css",
        _ => "application/octet-stream",
    }
}

fn serve_file(stream: &mut TcpStream, web: &Path, path: &str) {
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
    let bad = rel.split('/').any(|c| c == ".." || c.starts_with('.')) || rel.contains('\\');
    let full: PathBuf = web.join(rel);
    let body = if bad { None } else { std::fs::read(&full).ok() };
    match body {
        Some(b) => {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                content_type(&full),
                b.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&b);
        }
        None => {
            let msg = "Not found";
            let _ = stream.write_all(format!("HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", msg.len(), msg).as_bytes());
        }
    }
}

fn handle_conn(mut stream: TcpStream, lobby: Arc<Mutex<Lobby>>, web: PathBuf) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut req = Vec::new();
    let mut buf = [0u8; 2048];
    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => req.extend_from_slice(&buf[..n]),
        }
        if req.len() > 16384 {
            return;
        }
    }
    let head_end = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let head = String::from_utf8_lossy(&req[..head_end]).to_string();
    let leftover = req[head_end..].to_vec();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or("");
    let mut it = first.split_whitespace();
    let method = it.next().unwrap_or("");
    let path = it.next().unwrap_or("/").to_string();
    let mut key = None;
    let mut upgrade = false;
    for l in lines {
        let lower = l.to_ascii_lowercase();
        if lower.starts_with("sec-websocket-key:") {
            key = Some(l[18..].trim().to_string());
        }
        if lower.starts_with("upgrade:") && lower.contains("websocket") {
            upgrade = true;
        }
    }
    if !(upgrade && path.starts_with("/ws")) {
        if method == "GET" {
            serve_file(&mut stream, &web, &path);
        }
        return;
    }
    let key = match key {
        Some(k) => k,
        None => return,
    };
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key(&key)
    );
    if stream.write_all(resp.as_bytes()).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_nodelay(true);

    // writer thread
    let (tx, rx) = channel::<Vec<u8>>();
    let mut wstream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    std::thread::spawn(move || {
        for frame in rx {
            if wstream.write_all(&frame).is_err() {
                break;
            }
        }
        let _ = wstream.shutdown(std::net::Shutdown::Both);
    });

    let id = {
        let mut l = lobby.lock().unwrap();
        l.next_id += 1;
        let id = l.next_id;
        l.clients.insert(id, Client { name: format!("PILOT{}", id), tx: tx.clone(), room: None });
        id
    };

    let mut fr = FrameReader::default();
    fr.push(&leftover);
    'outer: loop {
        while let Some(f) = fr.next() {
            match f {
                Err(()) => break 'outer,
                Ok((OP_CLOSE, _)) => {
                    let _ = tx.send(encode_frame(OP_CLOSE, &[], None));
                    break 'outer;
                }
                Ok((OP_PING, p)) => {
                    let _ = tx.send(encode_frame(OP_PONG, &p, None));
                }
                Ok((OP_TEXT, p)) => {
                    let text = String::from_utf8_lossy(&p).to_string();
                    lobby.lock().unwrap().handle(id, &text);
                }
                Ok(_) => {}
            }
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => fr.push(&buf[..n]),
        }
    }
    let mut l = lobby.lock().unwrap();
    l.leave(id);
    l.clients.remove(&id);
}

/// Starts the server in background threads; returns the bound address.
pub fn start(addr: &str, web: PathBuf) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(12345) | 1;
    let lobby = Arc::new(Mutex::new(Lobby { seed, ..Default::default() }));
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let lobby = lobby.clone();
            let web = web.clone();
            std::thread::spawn(move || handle_conn(s, lobby, web));
        }
    });
    Ok(local)
}

/// Finds the browser build folder: ./web, or web/ next to the program or up to four folders above it.
pub fn find_web_dir() -> PathBuf {
    let mut cands = vec![PathBuf::from("web")];
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            cands.push(up.join("web"));
        }
    }
    cands.into_iter().find(|p| p.join("index.html").exists()).unwrap_or_else(|| "web".into())
}
