use teloxide::{
    dispatching::dialogue::{GetChatId, InMemStorage},
    payloads::SendMessageSetters,
    prelude::Dialogue,
    requests::Requester,
    types::{
        CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId, ReplyMarkup
    },
    Bot,
};

use crate::HandlerResult;

#[derive(Default, Clone, Debug)]
pub enum CardState {
    #[default]
    Start,
    ChooseCardOption,
    GiveCard {
        /// ID of the message querying the target of the /carte.
        /// Used to delete the message after the selection.
        message_id: MessageId,
    },
    // ReturnCard {
    //     /// ID of the message.
    //     /// Used to delete the message after the selection.
    //     message_id: MessageId,
    // },
}
pub type CarteDialogue = Dialogue<CardState, InMemStorage<CardState>>;


// when /carte: message with where the card is (either in office, either in CLIC or the name of the holder)
// if in CLIC, propose to give the card, or to do nothing
// if a holder have the card, propose to give back the card to the office or do nothing


/// Starts the /carte dialogue.
pub async fn start_card_dialogue(
    bot: Bot,
    msg: Message,
    dialogue: CarteDialogue,
) -> HandlerResult {
    log::info!("Starting /carte dialogue");

    // log::debug!("Removing /carte message");
    // bot.delete_message(msg.chat.id, msg.id).await?;

    // if there is a holder:
    let cartd_status = "CLIC"; // TODO get DB info from who hold the card

    let is_at_office = cartd_status == "CLIC";
    let (text, token) = if is_at_office {("Donner la carte", "give_card")} else {("Rendre la carte", "return_card")};
    
    let row = vec![
        InlineKeyboardButton::callback(text, token),
        InlineKeyboardButton::callback("Cancel", "nothing")
    ];

    log::debug!("Sending message with inline keyboard for callback");

    bot
        .send_message(msg.chat.id, cartd_status)
        .reply_markup(ReplyMarkup::InlineKeyboard(InlineKeyboardMarkup::new(vec![row])))
        .await?;

    log::debug!("Updating dialogue to ChooseTarget");
    dialogue
        .update(CardState::ChooseCardOption)
        .await?;

    Ok(())
}


/// Handles the callback from the inline keyboard, and sends a message to confirm selection of option.
/// The CallbackQuery data contains the action to perform.
pub async fn choose_option(
    bot: Bot,
    callback_query: CallbackQuery,
    dialogue: CarteDialogue,
    message_id: MessageId,
) -> HandlerResult {
    if let Some(id) = callback_query.chat_id() {

        log::debug!("Removing option query message");
        bot.delete_message(dialogue.chat_id(), message_id).await?;

        log::debug!("Sending target selection message");
        
        return match callback_query.data.unwrap_or_default().as_str() {
            "give_card" =>  {
                
                let msg = bot.send_message(id, "Qui prend la carte ?").await?;
                dialogue.update(CardState::GiveCard { message_id: msg.id }).await?;
                Ok(())
            },
            "return_card" => return_card(bot, id).await,
            _ => Ok(()), // TODO close the dialogue
        }
    }

    Ok(())
}


pub async fn give_card(
    bot: Bot,
    msg: Message,
    dialogue: CarteDialogue
) -> HandlerResult {

    let card_holder = msg.text();
    
    // TODO set holder in the db
    bot.send_message(dialogue.chat_id(), "{} est désormais en posséssion de la carte invité").await?;


    Ok(())
}

async fn return_card(bot: Bot, chat_id: ChatId) -> HandlerResult {
    bot.send_message(chat_id, "{} a rendu la carte au bureau !").await?;
    Ok(()) // TODO change state in the db to bureau
}