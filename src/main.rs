extern crate derive_new;
#[macro_use]
extern crate derive_builder;

mod client;
mod server;

mod logger;
mod settings;
mod version;
mod message;
mod encoding;
mod util;
mod direction;
mod vec2;
mod character;
mod ids;
mod specification;

use client::configuration::{Start};

use clap::{Arg, ArgAction, Command};

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(
            "Terminal deathmatch: pick a letter and be the last one standing in the arena.\n\
             With no command it opens the menu: play against computer players, \
             or host or join a game on the local network.",
        )
        .args(logger::global_args())
        .arg(
            Arg::new("demo")
                .long("demo")
                .action(ArgAction::SetTrue)
                .help("Watch the computer play every letter, match after match"),
        )
        .subcommand(client::solo_cli())
        .subcommand(client::configure_cli())
        .subcommand(server::configure_cli())
}

fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("client", matches)) => client::run(matches, Start::Join),
        Some(("server", matches)) => server::run(matches),
        Some(("solo", matches)) => client::run(matches, Start::Solo),
        _ => match matches.get_flag("demo") {
            true => client::run(&matches, Start::Demo),
            false => client::run(&matches, Start::Home),
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_line_is_well_formed() {
        super::cli().debug_assert();
    }
}
