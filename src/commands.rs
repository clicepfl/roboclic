use log::error;
use teloxide::{
    dispatching::DpHandlerDescription, prelude::*, types::Message, utils::command::BotCommands, Bot,
};

use crate::{config::config, HandlerResult};

pub use self::poll::PollState;
use self::poll::{poll_callback_query_handler, poll_message_handler};

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
                "Command {} refused for chat {}",
                command.shortand(),
                msg.chat.id
            );
            log::error!(
                "Command {} refused for chat {}",
                command.shortand(),
                msg.chat.id
            );
        }

        authorized
    })
}

/// Handle an incoming command. This does not verify whether the command is authorized.
///
/// Required dependencies: `teloxide_core::types::message::Message`, `roboclic_v2::commands::Command`, `teloxide_core::bot::Bot`
///
/// Note:  The command /poll is handled by the submodule `poll`, since in requires a more complex dialogue
async fn handle_command(bot: Bot, msg: Message, cmd: Command) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Bureau => {
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
        }
        Command::Poll => error!("UNREACHABLE"),
    };

    Ok(())
}

pub fn command_message_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry().branch(poll_message_handler()).branch(
        dptree::entry()
            .filter_command::<Command>()
            .chain(verify_authorization())
            .endpoint(handle_command),
    )
}

pub fn command_callback_query_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry().branch(poll_callback_query_handler())
}

mod poll {
    use teloxide::{
        dispatching::{
            dialogue::{GetChatId, InMemStorage},
            DpHandlerDescription, HandlerExt,
        },
        dptree,
        payloads::{SendMessageSetters, SendPollSetters},
        prelude::{DependencyMap, Dialogue, Endpoint},
        requests::Requester,
        types::{
            CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId,
            ReplyMarkup,
        },
        Bot,
    };

    use crate::{config::config, HandlerResult};

    use super::{verify_authorization, Command};

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
    type PollDialogue = Dialogue<PollState, InMemStorage<PollState>>;

    /// Starts the /poll dialogue by sending a message with an inline keyboard to select the target of the /poll.
    async fn start_poll_dialogue(bot: Bot, msg: Message, dialogue: PollDialogue) -> HandlerResult {
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
    async fn choose_target(
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
    async fn set_quote(
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

            let split_pos = (config().committee.len() / 2) as u8;

            // Splits the committee and add an option to refer to the other poll.
            let polls = config().committee.split_at(split_pos as usize);
            let polls = (
                [polls.0, &["J'ai voté en dessous".to_owned()]].concat(),
                [polls.1, &["J'ai voté en dessus".to_owned()]].concat(),
            );

            let target = config()
                .committee
                .iter()
                .enumerate()
                .find_map(|(i, s)| (*s == target).then_some(i as u8))
                .unwrap_or_default();

            log::debug!("Sending first poll");
            bot.send_poll(
                dialogue.chat_id(),
                format!(r#"Qui a dit: "{}" ?"#, text),
                polls.0,
            )
            .type_(teloxide::types::PollType::Quiz)
            .is_anonymous(false)
            .correct_option_id(if target < split_pos {
                target
            } else {
                split_pos
            })
            .await?;

            log::debug!("Sending second poll");
            bot.send_poll(
                dialogue.chat_id(),
                format!(r#"Qui a dit: "{}" ?"#, text),
                polls.1,
            )
            .type_(teloxide::types::PollType::Quiz)
            .is_anonymous(false)
            .correct_option_id(if target >= split_pos {
                target - split_pos
            } else {
                config().committee.len() as u8 - split_pos
            })
            .await?;

            log::debug!("Resetting dialogue status");
            dialogue.update(PollState::Start).await?;
        }

        Ok(())
    }

    pub fn poll_message_handler(
    ) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
            .branch(
                dptree::entry()
                    .filter_command::<Command>()
                    .chain(verify_authorization())
                    .filter(|c: Command| matches!(c, Command::Poll { .. }))
                    .endpoint(start_poll_dialogue),
            )
            .branch(dptree::case![PollState::SetQuote { message_id, target }].endpoint(set_quote))
    }
    pub fn poll_callback_query_handler(
    ) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::case![PollState::ChooseTarget { message_id }].endpoint(choose_target)
    }
}
