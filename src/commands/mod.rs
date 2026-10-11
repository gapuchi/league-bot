mod draft;
mod helpers;
mod meta;
mod pick_player;
mod registration;
mod season;
mod standings;
mod wc;

pub use draft::draft;
pub use meta::{help, ping, register, version};
pub use pick_player::pick_player;
pub use registration::{assign, claim, my_team, teams, unclaim, undrafted};
pub use season::season;
pub use standings::standings;
pub use wc::remaining;

use crate::league::League;

pub fn all() -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    let mut commands = vec![
        ping(),
        version(),
        help(),
        register(),
        season(),
        draft(),
        claim(),
        assign(),
        unclaim(),
        my_team(),
        teams(),
        undrafted(),
        standings(),
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
        League::Epl | League::Nfl | League::Nba => vec![],
    }
}
