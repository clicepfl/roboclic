use teloxide::{
    dispatching::DpHandlerDescription, prelude::*, types::Message, utils::command::BotCommands, Bot,
};

use crate::{config::config, HandlerResult};

pub use self::poll::PollState;

pub fn command_message_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .chain(verify_authorization())
                .branch(dptree::case![Command::Help].endpoint(help))
                .branch(dptree::case![Command::Bureau].endpoint(bureau))
                .branch(dptree::case![Command::Poll].endpoint(poll::start_poll_dialogue)),
        )
        .branch(dptree::case![PollState::SetQuote { message_id, target }].endpoint(poll::set_quote))
}

pub fn command_callback_query_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::case![PollState::ChooseTarget { message_id }].endpoint(poll::choose_target)
}

/// Check that the chat from which a command originated as the authorization to use it
///
/// Required dependencies: `teloxide_core::types::message::Message`, `roboclic_v2::commands::Command`
fn verify_authorization() -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry().filter(|command: Command, msg: Message| {
        let authorized =
            if let Some(authorized_chats) = config().access_control.get(command.shortand()) {
                authorized_chats.contains(&msg.chat.id.0)
            } else {
                true
            };

        if !authorized {
            log::info!(
                "Command /{} refused for chat {}",
                command.shortand(),
                msg.chat.id
            );
        }

        authorized
    })
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Crée un sondage pour savoir qui est au bureau")]
    Bureau,
    #[command(description = "Crée un quiz sur une citation d'un des membres du comité")]
    Poll,
}

impl Command {
    // Used as key for the access control map
    pub fn shortand(&self) -> &str {
        match self {
            Self::Help => "help",
            Self::Bureau => "bureau",
            Self::Poll => "poll",
        }
    }
}

async fn help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

async fn bureau(bot: Bot, msg: Message) -> HandlerResult {
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

mod poll {
    use rand::{seq::SliceRandom, thread_rng, Rng};
    use teloxide::{
        dispatching::dialogue::{GetChatId, InMemStorage},
        payloads::{SendMessageSetters, SendPollSetters},
        prelude::Dialogue,
        requests::Requester,
        types::{
            CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId,
            ReplyMarkup,
        },
        Bot,
    };

    use crate::{config::config, HandlerResult};

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
    pub async fn start_poll_dialogue(
        bot: Bot,
        msg: Message,
        dialogue: PollDialogue,
    ) -> HandlerResult {
        log::info!("Starting /poll dialogue");

        log::debug!("Removing /poll message");
        bot.delete_message(msg.chat.id, msg.id).await?;

        log::debug!("Sending message with inline keyboard for callback");
        let msg = bot
            .send_message(msg.chat.id, "Qui l'a dit ?")
            .reply_markup(ReplyMarkup::InlineKeyboard(InlineKeyboardMarkup::new(
                config()
                    .committee
                    .iter()
                    .map(|s| {
                        InlineKeyboardButton::new(
                            s,
                            teloxide::types::InlineKeyboardButtonKind::CallbackData(s.to_owned()),
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

            let split_pos = 10 as u8; // max poll options

            // Splits the committee to have only 10 answers possible.
            let mut poll = config().committee.clone();
            poll.retain(|s| -> bool {*s != target});    // filter the target from options
            poll.shuffle(&mut thread_rng());                // shuffle the options
            let index = thread_rng().gen_range(0..split_pos); // generate a valid index to insert target back
            poll.insert(index as usize, target.clone());        // insert target back in options
            let polls = poll.split_at(split_pos as usize);  // split options to have only 10 options

            let target = config()
                .committee
                .iter()
                .enumerate()
                .find_map(|(i, s)| (*s == target).then_some(i as u8))
                .unwrap_or_default();

            log::debug!("Sending poll");
            bot.send_poll(
                dialogue.chat_id(),
                format!(r#"Qui a dit: "{}" ?"#, text),
                polls.0.to_vec(),
            )
            .type_(teloxide::types::PollType::Quiz)
            .is_anonymous(false)
            .correct_option_id(if target < split_pos {
                target
            } else {
                split_pos
            })
            .await?;

            log::debug!("Resetting dialogue status");
            dialogue.update(PollState::Start).await?;
        }

        Ok(())
    }
}
