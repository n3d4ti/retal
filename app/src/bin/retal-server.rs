//! Retal match server. Run it on any computer the players can reach:
//!   retal-server [--port 8787] [--web path/to/web]
//! Then open http://<that-computer>:8787/ in a browser (or start the desktop game with
//! --server ws://<that-computer>:8787/ws) and choose MULTIPLAYER.

#[path = "../wsproto.rs"]
mod wsproto;
#[path = "../server.rs"]
mod server;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port = server::DEFAULT_PORT;
    let mut web: Option<std::path::PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().unwrap_or(port);
                i += 1;
            }
            "--web" if i + 1 < args.len() => {
                web = Some(args[i + 1].clone().into());
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let web = web.unwrap_or_else(server::find_web_dir);
    match server::start(&format!("0.0.0.0:{}", port), web.clone()) {
        Ok(addr) => {
            println!("Retal server running on port {}", addr.port());
            println!("  browser game:   http://<this-computer>:{}/   (serving {})", addr.port(), web.display());
            println!("  desktop game:   retal --server ws://<this-computer>:{}/ws", addr.port());
            println!("Press Ctrl+C to stop.");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        Err(e) => {
            eprintln!("Could not start the server on port {}: {}", port, e);
            std::process::exit(1);
        }
    }
}
