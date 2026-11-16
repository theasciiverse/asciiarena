# Changes from upstream

This is the Asciiverse edition of AsciiArena by lemunozm (Apache-2.0),
continuing from 0.0.7, upstream's master at c278865 (2022-06-07). The
`upstream` remote is https://github.com/lemunozm/asciiarena; the
`asciiverse` branch starts from it. DESIGN.md explains the design.

## Brought up to date

- Rust 2021 edition, Rust 1.88 or newer; builds without warnings.
- tui 0.15 to ratatui 0.30, crossterm 0.20 to 0.29, clap 2 to 4,
  message-io 0.13 to 0.19 (connections now report through the `Connected`
  event), bincode 1 to 2, rand 0.8 to 0.9, fern to env_logger,
  lazy_static and chrono dropped.

## Fixes

- Casting a fire ball while facing the outer wall could panic the server
  (`Map::terrain` asserted on a cell outside the map). Spells are no longer
  created inside walls, and anything outside the map counts as wall.
- A player who stepped into a fire ball, or swapped cells with it, took no
  damage. One collision pass now covers both.
- The client unwrapped a connection that could be gone (a key pressed after
  the server was lost) and an arena that could be missing (a game ending
  before one started); both are handled.
- On Windows, crossterm reports key releases too; they no longer act twice.
- A panic restores the terminal before it reports.

## New

- Five skills with energy and cooldowns, five arenas, the storm, rounds and
  a final table, a kill feed (DESIGN.md).
- Computer players at three levels, on the server, for solo and LAN games.
- A main menu: Play solo, Host a LAN game, Join a LAN game, Controls.
- `asciiarena solo`, `asciiarena --demo`, and match options on `server`
  (`--bots`, `--level`, `--rounds`, `--points`, `--arena`, `--storm`).
- Settings remembered in `$XDG_STATE_HOME/asciiarena`.
- Session logging: `RUST_LOG=debug asciiarena 2>log` logs every key, every
  change of screen and game, and a snapshot of the arena every half second,
  from the client and from a server running inside it.
- Tests: spells, skills, the storm, kill credit, arenas, points and rounds,
  bots, and whole bot matches played headless.

## Packaging (new files)

- `.github/workflows/release.yml`: static Linux builds (x86_64, aarch64),
  macOS (Apple silicon, Intel) and Windows, archives named for av, and
  SHA256SUMS.
- `asciiverse/record-preview.sh`: records the catalog preview from
  `asciiarena --demo`.
- `asciiverse/asciiarena.toml` and `asciiverse/asciiarena.md`: the catalog
  manifest and page.
- README.md rewritten for players; DESIGN.md, UPSTREAM.md and this file.
