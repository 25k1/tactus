use super::*;

pub fn chart_label(region: &str) -> String {
    match region {
        "russia" => fl!("chart-russia"),
        "world" => fl!("chart-world"),
        "kazakhstan" => fl!("chart-kazakhstan"),
        "belarus" => fl!("chart-belarus"),
        "uzbekistan" => fl!("chart-uzbekistan"),
        "georgia" => fl!("chart-georgia"),
        region => region.to_owned(),
    }
}

pub fn station_category(code: &str) -> String {
    match code {
        "genre" => fl!("station-genres"),
        "mood" => fl!("station-moods"),
        "activity" => fl!("station-activities"),
        "epoch" => fl!("station-eras"),
        "local" => fl!("station-local"),
        code => code.to_owned(),
    }
}

pub fn section_label(kind: SectionKind, title: &str) -> String {
    match kind {
        SectionKind::Best => fl!("section-all"),
        SectionKind::Tracks => fl!("section-tracks"),
        SectionKind::Albums => fl!("section-albums"),
        SectionKind::AppearsOn => fl!("section-appears-on"),
        SectionKind::Artists => fl!("section-artists"),
        SectionKind::SimilarArtists => fl!("section-similar-artists"),
        SectionKind::Playlists => fl!("section-playlists"),
        SectionKind::Podcasts => fl!("section-podcasts"),
        SectionKind::Episodes => fl!("section-episodes"),
        SectionKind::Suggestions => fl!("section-suggestions"),
        SectionKind::DislikedTracks => fl!("section-disliked-tracks"),
        SectionKind::DislikedArtists => fl!("section-disliked-artists"),
        SectionKind::Stations => station_category(title),
        SectionKind::Similar => fl!("section-similar"),
        SectionKind::Versions => fl!("section-versions"),
        SectionKind::Editions => fl!("section-editions"),
        SectionKind::Links => fl!("section-links"),
        SectionKind::Charts => fl!("section-charts"),
        SectionKind::FollowedReleases => fl!("section-followed-releases"),
        SectionKind::GenreReleases => fl!("section-genre-releases"),
        SectionKind::RecommendedArtists => fl!("section-recommended-artists"),
        SectionKind::Genres => fl!("section-genres"),
        SectionKind::Pinned => fl!("section-pinned"),
        SectionKind::Other => title.to_owned(),
        SectionKind::NewEpisodes if title.is_empty() => fl!("section-new-episodes"),
        SectionKind::NewEpisodes => format!("{}: {title}", fl!("section-new-episodes")),
    }
}
