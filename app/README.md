# Retal

Endless arcade air combat. Weapons never run out — each has a cooldown — and enemy waves keep coming.

## Computer
`cargo run --release`

## Browser and phone
1. Run `./build_web.sh` (it refreshes `web/retal.wasm`).
2. Put the `web/` folder on any static web host. Phones only give the tilt sensor to **https** pages.
3. Open `index.html`, tap START (iPhone asks for motion-sensor permission — allow it), hold the phone sideways at a comfortable angle and tap to fly. That angle is "level"; the LEVEL button re-centres it.

Phone controls: tilt to steer, hold GUN, tap MSL, WPN switches missile, FLR drops flares, left slider is throttle. Without a tilt sensor, drag on the left of the screen to steer.

## Difficulty
Choose EASY, NORMAL or HARD on the title screen (also on the pause and game-over screens). Easier settings make the enemy pilots slower and less accurate and make your guns and missiles easier to use: faster locks, shorter cooldowns, gun aim assist and missiles that home even before a full lock. The radar missile is selected when you take off.

## Multiplayer
Choose MULTIPLAYER on the title screen. CREATE MATCH lets you name it, make it PUBLIC (anyone can join from the list) or PRIVATE (others need the password), and pick CO-OP (team up against the waves) or DOGFIGHT (everyone shoots everyone, bots keep flying). Others tap your match in the list, or use JOIN BY CODE with the 4-letter code shown in your pause menu.

Everyone must reach the same Retal server:
- **Desktop game:** if no server is running on the computer, the game starts one itself (port 8787). Friends on the same network start their game with `--server ws://<your-ip>:8787/ws`, or open `http://<your-ip>:8787/` in a browser.
- **Browser / phones:** run `retal-server` (`cargo run --release --bin retal-server`) on a computer everyone can reach. It serves the browser game and the matches on port 8787. For phones over the internet put it behind HTTPS (the tilt sensor needs it); the game then uses `wss://` automatically.
