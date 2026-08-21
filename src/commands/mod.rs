mod config;
mod draft;
mod helpers;
mod meta;
mod nfl;
mod pick_player;
mod registration;
mod season;
mod wc;

pub use config::{config, config_channel, config_league, config_leagues};
pub use draft::draft;
pub use meta::{help, ping, register, version};
pub use pick_player::pick_player;
pub use registration::{assign, my_team, teams, unclaim, undrafted};
pub use season::season;
pub use wc::{remaining, standings};

use crate::league::League;

pub fn all() -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    let mut commands = vec![
        ping(),
        version(),
        help(),
        register(),
        config(),
        draft(),
        assign(),
        unclaim(),
        my_team(),
        teams(),
        undrafted(),
        standings(),
        season(),
        pick_player(),
    ];
    for league in League::ALL {
        commands.extend(commands_for(*league));
    }
    commands
}

/// League-specific slash commands. Exhaustive over `League` so new variants must register here.
fn commands_for(
    league: League,
) -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    match league {
        League::Wc => vec![remaining()],
        League::Epl => vec![],
    }
}
