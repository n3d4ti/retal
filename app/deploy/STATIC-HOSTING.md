# Hosting Retal as a static website

`public-html/` is the complete website: `index.html`, the game (`retal.wasm`), three small scripts and `config.js`.
It has no server code, so any static host works, and it runs from a sub-folder too (for example
`https://<you>.github.io/<repo>/`).

Rebuild it after changing the game: `./build_static.sh`

## GitHub Pages
Pick one:
- **Simplest:** commit `app/public-html/`, then in the repository go to Settings → Pages and either
  copy its contents to a `docs/` folder on your main branch and choose "Deploy from a branch → /docs",
  or use the workflow below.
- **Workflow:** copy `deploy/github-pages.yml` to `.github/workflows/pages.yml` in your repository and set
  Settings → Pages → Source to "GitHub Actions". Every push publishes `app/public-html`.

GitHub Pages uses https, so phone tilt controls work there.

## Multiplayer on a static site
A static host can't run the match server, so solo play works everywhere, and multiplayer needs a
Retal match server running somewhere (`cargo run --release --bin retal-server` on a computer or a
small cloud machine).

- Put its address in `public-html/config.js`, e.g. `var RETAL_SERVER = "wss://retal.example.com/ws";`
- A page on https (like GitHub Pages) can only use a **wss://** (secure) server address, so put the
  server behind HTTPS (for example with Caddy or nginx as a reverse proxy).
- Players can also type an address in the lobby (SERVER ADDRESS), or open the page as
  `index.html?server=wss://retal.example.com/ws`. The last address that worked is remembered on each device.

## Phones
- Open the site on the phone, hold it sideways and tap START. iPhones ask for "Motion & Orientation" access:
  allow it, that is the tilt steering. The angle you hold the phone at when you tap FLY becomes level;
  the LEVEL button re-levels any time, and turning the phone re-levels automatically.
- Tilt too twitchy or too slow? Pause (II) and tap TILT: LOW / NORMAL / HIGH.
- "Add to Home Screen" (Share menu on iPhone, browser menu on Android) installs Retal as a full-screen app.
- Tilt only works on https pages; GitHub Pages is https.
- Phones draw a shorter, foggier view to keep the game smooth, and the screen stays on while you play.
