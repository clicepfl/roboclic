use log::error;
use reqwest::Client;
use serde::Deserialize;
use tokio::task::JoinSet;

use crate::config::config;

#[derive(Debug)]
pub enum Error {
    Request(reqwest::Error),
    Serde(serde_json::Error),
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Request(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

#[derive(Deserialize, Debug)]
pub struct Committee {
    pub id: i32,
    #[serde(rename = "surname")]
    pub name: String,
    pub poll_count: i32,
}

#[derive(Deserialize, Debug)]
struct DirectusResponse<T> {
    data: T,
}

pub async fn get_committee() -> Result<Vec<Committee>, Error> {
    #[derive(Deserialize, Debug)]
    struct Member {
        member: Committee,
    }

    let response = Client::new()
        .get(format!(
            "{}/items/association_memberships?fields=member.id,member.surname,member.poll_count",
            config().directus_url
        ))
        .bearer_auth(&config().directus_token)
        .send()
        .await?
        .error_for_status()?;

    let response =
        serde_json::from_str::<DirectusResponse<Vec<Member>>>(response.text().await?.as_str())?;

    Ok(response.data.into_iter().map(|m| m.member).collect())
}

pub async fn update_committee(committee: Vec<Committee>) {
    let mut set = JoinSet::new();
    for c in committee {
        set.spawn(
            Client::new()
                .patch(format!("{}/items/members/{}", config().directus_url, c.id))
                .bearer_auth(&config().directus_token)
                .header("Content-Type", "application/json")
                .body(format!(r#"{{ "poll_count": {} }}"#, c.poll_count))
                .send(),
        );
    }

    while let Some(r) = set.join_next().await {
        match r {
            Err(e) => error!("Join error while updating committee: {e:#?}"),
            Ok(Err(e)) => error!("Request error while updating committee: {e:#?}"),
            Ok(_) => {}
        }
    }
}
