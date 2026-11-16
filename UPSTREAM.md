# Notes for lemunozm

Hi Luis. This is AsciiArena finished as a game, made for Asciiverse
(asciiverse.sh), a catalog and launcher for terminal games. It keeps your
architecture and your protocol design, and it is yours to take in any part,
or not. Everything is Apache-2.0, as yours is.

## How it is laid out

The changes follow the shape of your code, so they can be read file by file
against master (c278865):

- `specification/skills.rs` is new; `specification/spells.rs` grows the table.
- `server/game/arena.rs`: `update(now)` takes the time, `collide()` is the one
  place spells meet entities, `cast_skill()` carries out the five skills.
- `server/game/arena/spell.rs`: the behaviours (`Spark`, `Mine`, and still
  ones for walls, trails and blasts); `SpellAction::Cast` implemented.
- `server/game/bot.rs` is new; `player.rs` gains a `BotBehaviour` next to your
  `PlayerBehaviour`.
- `server/game.rs`: rounds, kills, a seeded generator, `step()` returns events.
- `server.rs`: `LocalServer` runs your `ServerManager` on a thread.
- `client/`: a home screen, the arena widgets extended, sessions in the state.

DESIGN.md lists what is yours and what was added; PATCHES.md lists fixes,
including two in the simulation (a panic when casting at the outer wall, and
fire balls that a player could walk through).

## Taking it in pieces

The dependency update stands on its own and could go first. The fixes in
`arena.rs` and `map.rs` are small. Skills, arenas and the storm need the
protocol fields listed at the end of DESIGN.md. Bots need only the server side.

## Not done

- Keys are fixed (WASD and arrows, Space Q E R F, 1 to 5); your README's
  "custom keys configuration" is still open.
- No discovery of games on the local network; joining takes the address.
- Rustfmt: the code follows your `.rustfmt.toml` by hand, as its options need
  a nightly rustfmt.
