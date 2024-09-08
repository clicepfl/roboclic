const POLL_MAX_OPTIONS_COUNT: u8 = 10; // max poll options

use crate::directus::{get_committee, update_committee, Committee};
use log::error;
use rand::{seq::SliceRandom, thread_rng, Rng};
use teloxide::{
    dispatching::dialogue::{GetChatId, InMemStorage},
    payloads::{SendMessageSetters, SendPollSetters},
    prelude::Dialogue,
    requests::Requester,
    types::{
        CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId, ReplyMarkup,
    },
    Bot,
};

use crate::HandlerResult;

#[derive(Default, Clone, Debug)]
pub enum PollState {
    #[default]
    Start,
    ChooseTarget {
        /// ID of the message querying the target of the /poll.
        /// Used to delete the message after the selection.
        message_id: MessageId,
    },
    SetQuote {
        /// ID of the message querying the quote.
        /// Used to delete the message after the selection.
        message_id: MessageId,
        target: String,
    },
}
pub type PollDialogue = Dialogue<PollState, InMemStorage<PollState>>;

/// Starts the /poll dialogue by sending a message with an inline keyboard to select the target of the /poll.
pub async fn start_poll_dialogue(bot: Bot, msg: Message, dialogue: PollDialogue) -> HandlerResult {
    log::info!("Starting /poll dialogue");

    log::debug!("Removing /poll message");
    bot.delete_message(msg.chat.id, msg.id).await?;

    let committee = match get_committee().await {
        Ok(v) => v,
        Err(e) => {
            error!("Could not fetch committee: {e:#?}");
            return Ok(());
        }
    };

    log::debug!("Sending message with inline keyboard for callback");
    let msg = bot
        .send_message(msg.chat.id, "Qui l'a dit ?")
        .reply_markup(ReplyMarkup::InlineKeyboard(InlineKeyboardMarkup::new(
            committee
                .into_iter()
                .map(|s| {
                    InlineKeyboardButton::new(
                        s.name.clone(),
                        teloxide::types::InlineKeyboardButtonKind::CallbackData(s.name),
                    )
                })
                .fold(vec![], |mut vec: Vec<Vec<InlineKeyboardButton>>, value| {
                    if let Some(v) = vec.last_mut() {
                        if v.len() < 3 {
                            v.push(value);
                            return vec;
                        }
                    }
                    vec.push(vec![value]);
                    vec
                }),
        )))
        .await?;

    log::debug!("Updating dialogue to ChooseTarget");
    dialogue
        .update(PollState::ChooseTarget { message_id: msg.id })
        .await?;

    Ok(())
}

/// Handles the callback from the inline keyboard, and sends a message to query the quote.
/// The CallbackQuery data contains the name of the target.
pub async fn choose_target(
    bot: Bot,
    callback_query: CallbackQuery,
    dialogue: PollDialogue,
    message_id: MessageId,
) -> HandlerResult {
    if let Some(id) = callback_query.chat_id() {
        log::debug!("Removing target query message");
        bot.delete_message(dialogue.chat_id(), message_id).await?;

        log::debug!("Sending quote query message");
        let msg = bot.send_message(id, "Qu'a-t'il/elle dit ?").await?;

        log::debug!("Updating dialogue to SetQuote");
        dialogue
            .update(PollState::SetQuote {
                message_id: msg.id,
                target: callback_query.data.unwrap_or_default(),
            })
            .await?;
    }

    Ok(())
}

/// Receives the quote and creates the poll. Since a poll can have at most 10 options,
/// it is split in two polls, each containing half of the comittee.
pub async fn set_quote(
    bot: Bot,
    msg: Message,
    dialogue: PollDialogue,
    (message_id, target): (MessageId, String),
) -> HandlerResult {
    if let Some(text) = msg.text() {
        log::debug!("Removing quote query message");
        bot.delete_message(dialogue.chat_id(), message_id).await?;
        log::debug!("Removing quote message");
        bot.delete_message(dialogue.chat_id(), msg.id).await?;

        let committee = match get_committee().await {
            Ok(v) => v,
            Err(e) => {
                error!("Could not fetch committee: {e:#?}");
                return Ok(());
            }
        };

        let mut poll = committee.iter().map(|c| c.name.clone()).collect::<Vec<_>>();

        // Splits the committee to have only 10 answers possible.
        poll.retain(|s| -> bool { *s != target }); // filter the target from options
        poll.shuffle(&mut thread_rng()); // shuffle the options
        let index = thread_rng().gen_range(0..(POLL_MAX_OPTIONS_COUNT - 1)); // generate a valid index to insert target back
        poll.insert(index as usize, target.clone()); // insert target back in options

        if poll.len() > POLL_MAX_OPTIONS_COUNT as usize {
            // split options to have only 10 options
            poll = poll.split_at(POLL_MAX_OPTIONS_COUNT as usize).0.to_vec();
        }

        log::debug!("Sending poll");
        bot.send_poll(
            dialogue.chat_id(),
            format!(r#"Qui a dit: "{}" ?"#, text),
            poll,
        )
        .type_(teloxide::types::PollType::Quiz)
        .is_anonymous(false)
        .correct_option_id(index)
        .await?;

        update_committee(
            committee
                .into_iter()
                .map(|c| {
                    if c.name == target {
                        Committee {
                            poll_count: c.poll_count + 1,
                            ..c
                        }
                    } else {
                        c
                    }
                })
                .collect(),
        )
        .await;

        log::debug!("Resetting dialogue status");
        dialogue.update(PollState::Start).await?;
    }

    Ok(())
}

pub async fn stats(bot: Bot, msg: Message) -> HandlerResult {
    let mut committee = match get_committee().await {
        Ok(v) => v,
        Err(e) => {
            error!("Could not fetch committee: {e:#?}");
            return Ok(());
        }
    };

    committee.sort_by_key(|r| r.poll_count);

    bot.send_message(
        msg.chat.id,
        committee
            .into_iter()
            .rev()
            .map(|c| format!("- {} (polls: {})", c.name, c.poll_count))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .await?;

    Ok(())
}
