use crate::media::MediaAppName;

#[test]
fn from_source_app_id_returns_readable_name() {
    let rows: [(Option<&str>, &str); 9] = [
        (Some("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "Media Player"),
        (Some("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify"),
        (Some("Contoso.PodcastPlayer_abc123!App"), "PodcastPlayer"),
        (Some("chrome.exe"), "Chrome"),
        (Some(r"C:\Program Files\VideoLAN\VLC\vlc.exe"), "VLC"),
        (Some("Spotify.exe"), "Spotify"),
        (Some("foobar2000.exe"), "Foobar2000"),
        (Some(""), ""),
        (None, ""),
    ];
    for (id, expected) in rows {
        assert_eq!(expected, MediaAppName::from_source_app_id(id), "{id:?}");
    }
}
