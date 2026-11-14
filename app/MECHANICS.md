# Mechanics sheet — Retal

Clean room: the original's files were not read, so every value below is **estimated** (designed for arcade feel from the person's request). Switch on Deep mode to compare with the original's real numbers.

Core rules from the request: weapons never run out (each one is governed only by a cooldown timer) and enemy waves respawn endlessly.

## Flight

| Value | Amount | Source |
|---|---|---|
| idle target speed | 100 m/s | estimated |
| military power target speed (90% throttle) | 330 m/s | estimated |
| afterburner target speed (100% throttle) | 420 m/s | estimated |
| max speed (diving) | 470 m/s | estimated |
| min speed | 60 m/s | estimated |
| stall speed | 95 m/s | estimated |
| speed response | 0.35 1/s | estimated |
| climb/dive speed exchange | 0.9 x g | estimated |
| throttle change rate | 0.6 per s | estimated |
| pitch rate up | 1.35 rad/s | estimated |
| pitch rate down | 0.95 rad/s | estimated |
| roll rate | 3.3 rad/s | estimated |
| rudder yaw rate | 0.35 rad/s | estimated |
| bank-to-turn rate at 90 deg bank | 0.55 rad/s | estimated |
| service ceiling | 15000 m | estimated |
| start altitude | 2000 m | estimated |
| start speed | 250 m/s | estimated |

## Camera

| Value | Amount | Source |
|---|---|---|
| cockpit vertical field of view | 55 deg | estimated |
| chase view field of view | 60 deg | estimated |
| chase camera distance | 38 m | estimated |
| view distance (fog end) | 16000 m | estimated |

## Weapons

| Value | Amount | Source |
|---|---|---|
| gun fire rate | 18 rounds/s | estimated |
| gun muzzle speed | 1100 m/s | estimated |
| gun range (life 1.4 s) | 1540 m | estimated |
| gun damage | 8 hp | estimated |
| gun spread | 0.004 rad | estimated |
| gun heat per shot | 3 % | estimated |
| gun cooling | 32 %/s | estimated |
| gun resumes after overheat at | 35 % | estimated |
| gun ammo | infinite | estimated |
| IR missile cooldown | 1.2 s | estimated |
| IR missile lock range | 3500 m | estimated |
| IR missile lock cone half-angle | 25 deg | estimated |
| IR missile lock time | 0.6 s | estimated |
| IR missile speed | 700 m/s | estimated |
| IR missile turn rate | 3.5 rad/s | estimated |
| IR missile flight time | 6 s | estimated |
| IR missile damage | 120 hp | estimated |
| radar missile cooldown | 4.0 s | estimated |
| radar missile lock range | 8000 m | estimated |
| radar missile lock cone half-angle | 40 deg | estimated |
| radar missile lock time | 1.2 s | estimated |
| radar missile speed | 800 m/s | estimated |
| radar missile turn rate | 2.2 rad/s | estimated |
| radar missile flight time | 12 s | estimated |
| radar missile damage | 120 hp | estimated |
| ground missile cooldown | 3.0 s | estimated |
| ground missile lock range | 6000 m | estimated |
| ground missile lock cone half-angle | 35 deg | estimated |
| ground missile lock time | 0.8 s | estimated |
| ground missile speed | 520 m/s | estimated |
| ground missile turn rate | 2.5 rad/s | estimated |
| ground missile flight time | 14 s | estimated |
| ground missile damage | 200 hp | estimated |
| missile payload | infinite (cooldown only) | estimated |
| missile blast radius | 30 m | estimated |
| proximity fuse | 14 m | estimated |
| flare cooldown | 5 s | estimated |
| flares per salvo | 6 | estimated |
| flare decoy chance per salvo | 0.75 | estimated |

## Health

| Value | Amount | Source |
|---|---|---|
| player hull | 100 hp | estimated |
| hull regen delay | 6 s | estimated |
| hull regen rate | 3 hp/s | estimated |
| lives at start | 3 | estimated |
| extra life every | 10000 points | estimated |
| respawn delay | 3 s | estimated |
| respawn shield | 3 s | estimated |

## Enemies

| Value | Amount | Source |
|---|---|---|
| fighter hp | 40 hp | estimated |
| fighter speed | 230 m/s | estimated |
| fighter turn rate | 0.95 rad/s | estimated |
| fighter gun range | 900 m | estimated |
| fighter gun interval | 0.125 s | estimated |
| fighter bullet damage | 3 hp | estimated |
| fighter missile cooldown | 12-18 s | estimated |
| ace hp | 70 hp | estimated |
| ace speed | 290 m/s | estimated |
| ace turn rate | 1.3 rad/s | estimated |
| ace gun range | 1000 m | estimated |
| ace gun interval | 0.1 s | estimated |
| ace bullet damage | 4 hp | estimated |
| ace missile cooldown | 8-12 s | estimated |
| enemy missile speed | 560 m/s | estimated |
| enemy missile turn rate | 2.1 rad/s | estimated |
| enemy missile damage | 35 hp | estimated |
| SAM range | 7000 m | estimated |
| SAM max target altitude | 6000 m | estimated |
| SAM cooldown | 10-14 s | estimated |
| SAM speed | 600 m/s | estimated |

## Waves

| Value | Amount | Source |
|---|---|---|
| wave kill quota | 4 + 2 x wave | estimated |
| enemies in the air at once | min(2 + wave, 8) | estimated |
| ground targets at once | min(4 + wave/2, 8) | estimated |
| ace chance | 15% per wave from wave 3, max 60% | estimated |
| enemy speed growth per wave | 3 % | estimated |
| spawn interval | 2.5 s | estimated |
| spawn distance | 5000-7000 m | estimated |
| pause between waves | 4 s | estimated |
| endless waves | True | estimated |

## Scoring

| Value | Amount | Source |
|---|---|---|
| fighter score | 100 + 25 x (wave-1) | estimated |
| ace score | 250 + 25 x (wave-1) | estimated |
| SAM score | 150 | estimated |
| tank score | 75 | estimated |
| hangar score | 50 | estimated |
| wave clear bonus | 500 x wave | estimated |

## Controls

| Value | Amount | Source |
|---|---|---|
| pitch | Arrows Up/Down or W/S (Down = pull up) | estimated |
| roll | Arrows Left/Right or A/D | estimated |
| rudder | Q / E | estimated |
| throttle | Shift / Ctrl (also + / -, PageUp/PageDown) | estimated |
| gun | Space or left mouse (hold) | estimated |
| missile | Enter, M or right mouse | estimated |
| weapon select | Tab / R cycle, 1-3 direct | estimated |
| target cycle | T | estimated |
| flares | F or X | estimated |
| view | V or C | estimated |
| pause | P or Esc | estimated |
| invert pitch | I or pause screen toggle | estimated |
| phone steering | tilt the phone (gyroscope); drag stick if no sensor | estimated |
| phone fire | GUN (hold) and MSL (tap) buttons | estimated |
| phone throttle | left slider | estimated |
| phone tilt for full roll | 26 deg | estimated |
| phone tilt for full pitch | 23 deg | estimated |
| phone tilt dead zone | 3 deg | estimated |

## Computer pilots

- Attack: fly toward a lead point on the player; fire guns within 900–1000 m and 6° of the nose; fire a missile when within range and 30° of the nose.
- Evade: when the player sits on their tail inside 1.3 km they break hard sideways/up for 2–4 s.
- Extend: after a head-on pass closer than 250 m they fly away 2.5–4 s, then come back.
- Keep 200 m apart, climb away from terrain under 500 m clearance, and are brought back if they stray beyond 16 km.
- SAM sites launch at the player inside 7 km when below 6000 m.

## Phone / browser

- The browser build (web/) uses the same rules. On a touch device the on-screen GUN, MSL, WPN, FLR, VIEW, pause and LEVEL buttons appear with a throttle slider on the left.
- Tilt is read from the phone's orientation sensor and measured relative to the screen, so it works in landscape either way round; the angle you hold the phone at when you tap to fly becomes level (LEVEL re-centres it).

## Difficulty (enemy bots and how easy your weapons are)

Picked on the title, pause and game-over screens (tap a box or press Left/Right). Default NORMAL. The radar missile is selected at the start of every flight.

| Setting | EASY | NORMAL | HARD |
|---|---|---|---|
| enemy gun spread multiplier | 2.0 | 1.0 | 0.65 |
| enemy turn rate multiplier | 0.75 | 1.0 | 1.15 |
| enemy speed multiplier | 0.88 | 1.0 | 1.08 |
| enemy damage multiplier (guns, missiles, SAMs) | 0.5 | 1.0 | 1.4 |
| enemy hp multiplier | 0.7 | 1.0 | 1.3 |
| enemy missile/SAM cooldown multiplier | 1.7 | 1.0 | 0.7 |
| enemy evade tendency multiplier | 0.4 | 1.0 | 1.8 |
| extra enemies in the air | -1 | 0 | 1 |
| player lock time multiplier | 0.4 | 1.0 | 1.3 |
| player lock cone multiplier | 1.5 | 1.0 | 0.85 |
| player missile cooldown multiplier | 0.6 | 1.0 | 1.25 |
| player missile turn multiplier | 1.4 | 1.0 | 0.9 |
| player gun hit radius | 15 m | 9 m | 7 m |
| player gun aim assist cone | 5.0 deg | 1.5 deg | 0.0 deg |
| fire-and-forget missiles (home before full lock) | True | False | False |
| score multiplier | 0.5 | 1.0 | 1.5 |

Effective radar missile cooldown: 2.4 s / 4.0 s / 5.0 s; radar lock time: 0.48 s / 1.2 s / 1.56 s. All values estimated.

## Multiplayer

| Value | Amount | Source |
|---|---|---|
| players per match | up to 8 | estimated |
| modes | Co-op (everyone vs the endless waves) / Dogfight (all vs all, bots keep flying) | person's request + estimated |
| access | public = anyone can join; private = password needed | person's request |
| match code | 4 letters | estimated |
| own aircraft sent | 15 times per second | estimated |
| host world snapshot | 10 times per second | estimated |
| respawns online | unlimited (no game over) | estimated |
| shooting down a pilot | 300 points x difficulty | estimated |
| bots | hunt the nearest living pilot; the host's computer runs them | estimated |
| match ends | when the host leaves | estimated |
