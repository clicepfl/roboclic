use teloxide::{payloads::SendPollSetters, requests::Requester, types::Message, Bot};

use crate::HandlerResult;

pub async fn bureau(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_poll(
        msg.chat.id,
        "Qui est au bureau ?",
        [
            "Je suis actuellement au bureau".to_owned(),
            "Je suis à proximité du bureau".to_owned(),
            "Je compte m'y rendre bientôt".to_owned(),
            "J'y suis pas".to_owned(),
            "Je suis à Satellite".to_owned(),
            "Je suis pas en Suisse".to_owned(),
        ],
    )
    .is_anonymous(false)
    .await?;
    Ok(())
}