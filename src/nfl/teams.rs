use crate::{
    api::EspnNflApi,
    league::CatalogTeam,
    types::{Data, Error},
};

pub async fn list_teams(data: &Data) -> Result<Vec<CatalogTeam>, Error> {
    let teams = EspnNflApi::new(data.http.clone()).fetch_teams().await?;
    Ok(teams
        .into_iter()
        .map(|team| CatalogTeam {
            id: team.id,
            name: team.display_name,
            short_name: team.short_display_name,
            code: team.abbreviation,
        })
        .collect())
}
