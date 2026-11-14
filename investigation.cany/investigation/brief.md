# Brief

**Question:** Build an original arcade jet-combat game in the spirit of a 1989 cockpit flight sim: infinite weapons on cooldown timers, endless respawning enemy waves; playable natively, in a browser and on phones (tilt to steer, on-screen buttons to shoot).

**Purpose:** Spiritual successor game for the person, who doesn't code.

**Targets:**
- /home/attila/retal (clean room, not opened)

**Constraints:**
- Clean room: nothing inside the original is read
- Rust + macroquad, cargo project in app/
- Web build (wasm) must run on phones with gyroscope + touch buttons

**Done when:** cargo build succeeds, --screenshot and --self-test pass, web build in app/web loads with tilt + touch controls, mechanics sheet written.
