# Pokémon Legends: Arceus (update-v262144) — N2 Weather, DressUp & Audio Report

## Overview
This report formalizes the reverse engineering and N2 promotion of the Weather & Time-of-Day engine, Player Customization (`MyDressup`), and the Dynamic Audio / SoundController subsystems under decision `dec311`.

---

## 1. Weather Simulation & Day/Night Cycle (`field::weather`, `gamesys::GameManager`)
- `FUN_024985e0`: Master weather simulation tick managing weather lottery, transition blends, and fog/snow density.
- `FUN_02499534`: Accessor for active weather type enum (clear, cloudy, rain, snow, blizzard, fog, intense sun).
- `FUN_024995bc`: Accessor for weather intensity and blend weight ($0.0 \dots 1.0$).
- `FUN_0249744c`: In-game clock accumulator evaluating time-of-day phases (morning, day, evening, night).

---

## 2. Player Customization & Clothing (`chara::DressUpData`, `savedata::io::MyDressup`)
- `savedata::io::MyDressup` manages visual customization across 12 distinct 64-bit FNV-1a hash slots located at offset `+0x68 + (index * 8)`.
  - Empty sentinel value: `0xcbf29ce484222645`.
  - Visual slots: Hair (0), Eye Contacts (1), Eyebrow Color (2), Glasses (3), Hat (4), Tops/Kimono (5), Bottoms/Trousers (6), Dress/Set (7), Shoes (8), Skin tone (11).
- Key routines:
  - `FUN_0101b748`: Slot reader from `MyDressup` memory layout.
  - `FUN_0101b7c0`: Slot writer updating player clothing hash.
  - `FUN_01152140`: Evaluates clothing hashes and swaps material textures / sub-meshes on the `PlayerActor`.

---

## 3. Dynamic Audio & SoundController (`gamesys::audio::SoundController`)
- Built on Audiokinetic Wwise middleware.
- Key routines:
  - `FUN_017ce974`: Posts Wwise audio event IDs to the SoundController queue.
  - `FUN_017cf7dc`: Master dynamic BGM transition coordinator (swapping between ambient area BGM, mount themes, stealth tension, Alpha chase, and combat fanfare).
  - `FUN_017ce4a4`: Sets Wwise Real-Time Parameter Control (RTPC) float values (e.g. distance to threat, underwater muffle, speed).

---

## 4. Promoted Functions Summary (10 Total)

| Address | Role | Description |
|---|---|---|
| `024985e0` | support | Master weather simulation tick and lottery. |
| `02499534` | support | Active weather type enum accessor. |
| `024995bc` | support | Weather intensity & blend weight float accessor. |
| `0249744c` | support | In-game clock accumulator & time-of-day phase evaluator. |
| `0101b748` | support | MyDressup 64-bit slot hash reader (+0x68 + idx*8). |
| `0101b7c0` | support | MyDressup 64-bit slot hash writer. |
| `01152140` | support | Applies visual customization slots to PlayerActor mesh. |
| `017ce974` | support | Posts Wwise audio event ID to SoundController queue. |
| `017cf7dc` | support | Dynamic BGM stream transition coordinator. |
| `017ce4a4` | support | Sets Wwise Real-Time Parameter Control (RTPC) float value. |
