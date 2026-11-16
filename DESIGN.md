# Design: what is lemunozm's, and what this edition adds

AsciiArena 0.0.7 had the bones of a full game and said so: "The final game
will include a lot of skills with custom keys configuration." This edition
fills it in along the lines already in the code, and leaves his structure
where it was.

## His, kept as it was

- **One program, three roles.** `asciiarena server`, `asciiarena client`, and
  a client by default. The server owns the game; clients only send keys and
  draw what they are told.
- **The protocol and its handshake** (`docs/design/communication.md`): version
  check, server info subscription, login by letter, a UDP channel trusted
  through the TCP session, game steps at 30 per second over UDP when it works.
- **message-io** for all of the networking, TCP framed plus UDP.
- **The client's architecture** (`docs/design/client.md`): input and server
  events queued to one thread, a store that turns actions into state, a GUI
  that draws the state. The lobby, its server info panel and the waiting room
  with the wandering letters are his, unchanged in behaviour.
- **The simulation model**: an `Arena` of entities and spells, each with a
  behaviour object (`EntityBehaviour`, `SpellBehaviour`) that returns actions
  for the arena to carry out. Walking speed per character, walking one cell at
  a time on key repeat.
- **The spell table** (`SpellSpec`: name, description, damage, speed,
  behaviour name) and the fire ball as an "explotable ball".
- **Health and energy** of 100 each, and the panels with his bars.
- **Points**: whenever players fall, everyone still standing gets one point
  per fallen player. `winner_points` ends a game early when reached.
- **Arenas in sequence**: a game is a run of arenas with a pause between them,
  and the open square with a wall around it is his (now called The Pit).

## Added here

- **Skills** (`specification/skills.rs`): the `SkillId` he already sent with
  every cast now picks one of five skills, each with an energy cost and a
  cooldown. Energy comes back at 15 points a second.
  - Fire ball (his), 10 damage, 7 energy, 0.25 s.
  - Dash: up to 4 cells at once, 20 energy, 1.5 s.
  - Nova: 8 sparks outward, 3 cells, 10 damage each, 30 energy, 2.5 s.
  - Wall: 3 stone cells for 5 s that stop walkers and spells, 25 energy, 4 s.
  - Mine: arms in 0.8 s, 25 damage, two per player, 20 energy, 2 s.
- **Spells that make spells**: `SpellAction::Cast`, a `todo!()` before, now
  creates new spells; the nova's sparks and the blasts of fire balls and mines
  come from it. `Create(Vec<Entity>)`, the other `todo!()`, was dropped as
  nothing needs it.
- **One collision pass** (`Arena::collide`): every spell touching an entity
  asks its behaviour what happens, run after entities act and again after
  spells move. Before, only a moving spell could hit, so a player stepping
  into a fire ball, or swapping cells with it, was not hurt.
- **Arenas**: Pillars, Crossroads, Ruins and Rings, 26 by 20 cells, each the
  same when turned half round, plus The Pit. A match rotates through them.
- **The storm**: after 40 s an arena starts closing from the walls, one ring
  every 2.5 s; standing in it costs 4 health every half second.
- **Rounds**: a match is N arenas (5 by default). The final table orders by
  his points, then kills, then arenas won. `winner_points` still works,
  and is off by default.
- **Kill credit**: each entity remembers who hit it last and with what, for
  the kill feed and the kills column.
- **Computer players** (`server/game/bot.rs`): a bot is a player whose
  behaviour thinks instead of reading keys. It reads the same arena view,
  returns the same `Walk` and `Cast` actions, and so walks at the same speed
  and pays the same costs. Each look: dodge what is flying at it (a side step,
  a dash, or a wall), pick a target, fire when lined up with a clear line,
  nova when someone is close, now and then lay a mine, then walk towards the
  best reachable cell (lined up with the target at a fair distance, out of
  the storm and of harm's way), found by a breadth-first search. Easy, Normal
  and Hard differ in how often they look, how likely they notice a shot, how
  well they aim, and which skills they use.
- **Solo and hosting from the client**: the client can start the server on a
  thread (`LocalServer`). Solo binds it to 127.0.0.1 on free ports; hosting
  binds 0.0.0.0 on 3549. Nothing touches the network otherwise.
- **The main menu** (home): solo, host, join, controls, and the match options,
  remembered in `$XDG_STATE_HOME/asciiarena/settings`.
- **Arena screen**: interior walls, colours per letter, compact panels for big
  matches, the skill bar with cooldowns, the storm, a kill feed, remains of
  the fallen, a leave prompt, and the final table.
- **Determinism for tests**: the arena and game take the time as an argument
  and draw randomness from a seeded generator, so a whole match between bots
  runs headless in the tests, and the same seed plays the same match.

## Protocol changes

The version is 0.1.0, so 0.0.x clients and servers refuse each other cleanly
through his version check. New fields: bots, level, rounds and arena in the
server info; per-player kills, rounds won and bot level in the game info;
arena name, size and storm start in the arena info; facing and cooldowns per
entity, owner per spell and the storm per frame; kill and arena-winner game
events; the final table with the end of the game.
