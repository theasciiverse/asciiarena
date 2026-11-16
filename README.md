# AsciiArena

**Twenty-six letters walk into an arena. One walks out.**

You are a letter. A capital one, bold and bright, standing on a floor of stone
with a handful of other letters who all want the same thing you do: to be the
last one standing. Pick yours, step in, and throw fire at everybody else.

When the arena empties, a new one opens, the fallen letters come back for
revenge, and the scores carry on. Five arenas later, somebody is the champion.

## The fight

Five skills, each on its own key, all paid for out of one bar of energy that
fills back up as you go:

- **Fire ball** (`Space`): flies where you face and bursts on the first thing it hits. Your bread and butter
- **Dash** (`Q`): a leap of four cells in a blink, straight past whatever was about to hit you
- **Nova** (`E`): eight sparks burst out of you in every direction. Murderous at close range
- **Wall** (`R`): three cells of stone rise in front of you for five seconds, and fire breaks on them
- **Mine** (`F`): left under your feet, armed in under a second, bursting under the next enemy foot

Every skill has its own cooldown, so a duel is never just who shoots first. It
is who runs dry first, who wastes the dash, who walks into the mine they
watched being laid.

And nobody gets to hide in a corner: forty seconds into every arena **the
storm** rolls in from the walls, swallowing the arena ring by ring. Anyone
caught in it burns. The circle closes, the letters are pushed together, and
somebody has to win.

**Five arenas**, a different one every round unless you pick one: **The Pit**,
the original open square; **Pillars**, nine stone columns to duck behind;
**Crossroads**, four quarters and a plaza; **Ruins**, broken walls everywhere;
and **Rings**, two walls around a heart you have to fight your way into.

**Scoring** rewards staying alive: every rival who falls while you are still on
your feet is a point for you, so the last one standing collects from everyone.
The most points after the last arena wins the match, and kills break ties.

## Play alone

Run `asciiarena` and choose **Play solo**. Up to seven computer players join
you, at three strengths:

- **Easy** bots are slow to notice a fire ball coming, and slow to aim one
- **Normal** bots dodge, dash out of trouble and lay mines
- **Hard** bots are quick and sharp, and raise walls against your fire

The bots play by your rules: they walk at your speed, pay the same energy, wait
out the same cooldowns, and see only what you see.

Want to watch first? `asciiarena --demo` hands every letter to the computer.

## Play together

Everyone on the same local network can join one match:

1. One of you chooses **Host a LAN game**. The screen shows the address to share.
2. The others choose **Join a LAN game**, type that address, pick a letter, and press Enter.
3. The match starts when everyone is in. Bots you set up in the menu fill out the arena.

A dedicated server without a screen works too: `asciiarena server -p 4 --bots 2`.
It listens on TCP and UDP port 3549.

## Controls

- `W` `A` `S` `D` or the **arrow keys**: walk, and turn to face that way
- `Space` or `1`: Fire ball
- `Q` or `2`: Dash
- `E` or `3`: Nova
- `R` or `4`: Wall
- `F` or `5`: Mine
- `Esc`: leave the match; `Ctrl+C` quits at once

In the menu: arrow keys to choose and change, `Enter` to go. On **Your
letter**, type the letter you want to be.

## Options

- `asciiarena`: the menu
- `asciiarena solo`: straight into a match against bots, with
  `--bots N`, `--level easy|normal|hard`, `--rounds N`,
  `--arena rotate|pit|pillars|crossroads|ruins|rings`, `--character X`,
  `--storm SECONDS` (until the storm), `--points N` (end the match at N points)
- `asciiarena --demo`: the computer plays every letter, match after match
- `asciiarena client [--host IP:PORT] [--character X]`: the lobby, to join a game
- `asciiarena server -p PLAYERS`: a server for the local network, with the
  same match options plus `--map-size N` (the side of The Pit),
  `--tcp-port`, `--udp-port`
- `--log LEVEL` and `--log-file FILE` on any of them

The menu remembers your letter and match in `$XDG_STATE_HOME/asciiarena`
(`~/.local/state/asciiarena`; `%LOCALAPPDATA%\asciiarena` on Windows). For a
session log, `RUST_LOG=debug asciiarena 2>asciiarena.log` records every key,
every change of screen and game, and a snapshot of the arena twice a second.

The arena is drawn for a 100x30 terminal and plays fully in 80x24.

## History

Luis Enrique Muñoz Martín (lemunozm) started AsciiArena at the end of June
2020: a deathmatch in the terminal, with a real client and server so
friends could fight over the network. In its first week he wrote the
networking for it, an event queue on top of mio, and then moved that work into
a library of its own. It became
[message-io](https://github.com/lemunozm/message-io), his event-driven network
library, which AsciiArena has used since July 2020 and which has gone on to
over two hundred thousand downloads.

The game itself got a menu, a login and a lobby, a waiting room where the
letters wander about, an arena and a points system, released as alpha versions
from November 2020 to 0.0.7 in April 2021. Its README promised "a lot of
skills with custom keys", but the full game stayed on the to-do list, with one
skill and the network demo working.

This edition, made for [Asciiverse](https://asciiverse.sh) in 2026, finishes
it in the direction he started: his spell and skill system, energy and points
now drive five skills, five arenas, the storm, rounds and a final table, and
computer players make it a game you can play alone. DESIGN.md tells what is
his and what is new, and PATCHES.md lists the changes.

## Building

```sh
cargo build --release
```

Rust 1.88 or newer. The binary is `target/release/asciiarena`; it is the game,
the client and the server at once. `cargo test` plays whole matches between
bots as part of the tests.

## Credits

- AsciiArena: Luis Enrique Muñoz Martín (lemunozm), 2020 to 2022
- Skills, arenas, storm, rounds and computer players: Asciiverse, 2026
- Built on [message-io](https://github.com/lemunozm/message-io),
  [ratatui](https://ratatui.rs) and [crossterm](https://github.com/crossterm-rs/crossterm)
- License: Apache-2.0 (see LICENSE)
