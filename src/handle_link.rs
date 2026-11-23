use reqwest::Url;
use teloxide::{
    Bot,
    requests::Requester,
    types::{Message, MessageEntityKind},
};

use crate::{
    HandlerResult,
    handle_link::KnownHost::{Bluesky, Instagram, Reddit, Spotify, Tiktok, Twitter, Youtube},
};

#[derive(Clone)]
enum KnownHost {
    Youtube(String),
    Twitter,
    Reddit,
    Instagram,
    Tiktok,
    Bluesky,
    Spotify,
}

impl TryFrom<String> for KnownHost {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "bsky.app" => Ok(Bluesky),
            "instagram.com" => Ok(Instagram),
            "reddit.com" | "redd.it" => Ok(Reddit),
            "spotify.com" | "open.spotify.com" => Ok(Spotify),
            "tiktok.com" | "vm.tiktok.com" => Ok(Tiktok),
            "twitter.com" | "x.com" => Ok(Twitter),
            "youtu.be" | "youtube.com" | "m.youtube.com" | "music.youtube.com" => {
                Ok(Youtube(value))
            }
            _ => Err("Could not parse provided string"),
        }
    }
}

impl Into<String> for KnownHost {
    fn into(self) -> String {
        match self {
            Bluesky => "bskx.app".into(),
            Instagram => "ddinstagram.com".into(),
            Reddit => "rxddit.com".into(),
            Spotify => "open.spotify.com".into(),
            Tiktok => "tntok.com".into(),
            Twitter => "fxtwitter.com".into(),
            Youtube(s) => s,
        }
    }
}

impl KnownHost {
    fn should_retain(&self, s: String) -> bool {
        let s = s.as_str();
        match self {
            Youtube(_) => s == "t" || s == "v" || s == "list",
            _ => false,
        }
    }
}

pub async fn rewrite_link(bot: Bot, msg: Message) -> HandlerResult {
    let entities = match msg.parse_entities() {
        Some(v) => v,
        _ => match msg.parse_caption_entities() {
            Some(v) => v,
            _ => vec![],
        },
    };

    let urls = entities
        .iter()
        .filter_map(|e| match e.kind().to_owned() {
            MessageEntityKind::Url => msg.text().map(|t| t.to_string()),
            MessageEntityKind::TextLink { url } => Some(url.to_string()),
            _ => None,
        })
        .filter_map(sanitize)
        .collect::<Vec<_>>();

    for u in urls {
        bot.send_message(msg.chat.id, format!("Sanitized link as {} :)", u))
            .await?;
    }
    Ok(())
}

pub fn sanitize(url: String) -> Option<String> {
    let url = if url.contains("://") {
        url
    } else {
        format!("https://{}", url)
    };

    let url = Url::parse(&url).ok()?;

    let path = url.path();

    let host = url.host_str()?;
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);

    let host = KnownHost::try_from(host.to_string()).ok()?;

    let params = url
        .query_pairs()
        .filter(|c| host.should_retain(c.clone().0.into_owned()))
        .collect::<Vec<_>>();

    if params.len() > 0 {
        Some(
            Url::parse_with_params(
                format!(
                    "https://{}{}",
                    <KnownHost as Into<String>>::into(host),
                    path
                )
                .as_str(),
                params,
            )
            .ok()?
            .into(),
        )
    } else {
        Some(
            Url::parse(
                format!(
                    "https://{}{}",
                    <KnownHost as Into<String>>::into(host),
                    path
                )
                .as_str(),
            )
            .ok()?
            .into(),
        )
    }
}
