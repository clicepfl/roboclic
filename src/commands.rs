use std::sync::Arc;

use sqlx::SqlitePool;
use teloxide::{
    dispatching::DpHandlerDescription, prelude::*, types::Message, utils::command::BotCommands, Bot,
};

use crate::{config::config, HandlerResult};

pub use self::poll::PollState;

const POLL_MAX_OPTIONS_COUNT: u8 = 10; // max poll options

pub fn command_message_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .branch(dptree::case![Command::Help].endpoint(help))
                .branch(dptree::case![Command::Authenticate(token, name)].endpoint(authenticate))
                .branch(
                    require_authorization()
                        .branch(dptree::case![Command::Bureau].endpoint(bureau))
                        .branch(dptree::case![Command::Poll].endpoint(poll::start_poll_dialogue)),
                )
                .branch(
                    require_admin().chain(
                        dptree::entry()
                            .branch(dptree::case![Command::AdminList].endpoint(admin_list))
                            .branch(
                                dptree::case![Command::AdminRemove(name)].endpoint(admin_remove),
                            )
                            .branch(dptree::case![Command::Authorize(command)].endpoint(authorize))
                            .branch(
                                dptree::case![Command::Unauthorize(command)].endpoint(unauthorize),
                            ),
                    ),
                ),
        )
        .branch(dptree::case![PollState::SetQuote { message_id, target }].endpoint(poll::set_quote))
}

pub fn command_callback_query_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::case![PollState::ChooseTarget { message_id }].endpoint(poll::choose_target)
}

// ----------------------------- ACCESS CONTROL -------------------------------

/// Check that the chat from which a command originated as the authorization to use it
///
/// Required dependencies: `teloxide_core::types::message::Message`, `roboclic_v2::commands::Command`
fn require_authorization() -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription>
{
    dptree::entry().filter_async(
        |command: Command, msg: Message, pool: Arc<SqlitePool>| async move {
            let chat_id = msg.chat.id.to_string();
            let shortand = command.shortand();
            match sqlx::query!(
                r#"SELECT COUNT(*) AS count FROM authorizations WHERE chat_id = $1 AND command = $2"#,
                chat_id,
                shortand
            )
            .fetch_one(pool.as_ref())
            .await {
                Ok(result) => result.count > 0,
                Err(e) => {
                    log::error!("Could not check authorization in database: {:?}", e);
                    false
                },
            }
        },
    )
}

/// Check that the chat is admin
///
/// Required dependencies: `teloxide_core::types::message::Message`, `sqlx_sqlite::SqlitePool`
fn require_admin() -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::entry().filter_async(|msg: Message, db: Arc<SqlitePool>| async move {
        let id = msg.chat.id.to_string();
        sqlx::query!(
            "SELECT COUNT(*) AS is_admin FROM admins WHERE telegram_id = $1",
            id
        )
        .fetch_one(db.as_ref())
        .await
        .is_ok_and(|r| r.is_admin > 0)
    })
}

// --------------------------- AVAILABLE COMMANDS -----------------------------

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
    #[command(
        description = "Authentifcation admin: /auth <token> <name>",
        parse_with = "split",
        separator = " "
    )]
    Authenticate(String, String),
    #[command(description = "(Admin) Liste les admins")]
    AdminList,
    #[command(description = "(Admin) Supprime un admin à partir de son nom")]
    AdminRemove(String),
    #[command(description = "(Admin) Authorise le groupe à utiliser la commande donnée")]
    Authorize(String),
    #[command(
        description = "(Admin) Révoque l'authorisation du groupe à utiliser la commande donnée"
    )]
    Unauthorize(String),
}

impl Command {
    // Used as key for the access control map
    pub fn shortand(&self) -> &str {
        match self {
            Self::Help => "help",
            Self::Bureau => "bureau",
            Self::Poll => "poll",
            Self::Authenticate(..) => "auth",
            Self::AdminList => "adminlist",
            Self::AdminRemove(..) => "adminremove",
            Self::Authorize(..) => "authorize",
            Self::Unauthorize(..) => "unauthorize",
        }
    }
}

// ---------------------------- COMMAND ENDPOINTS -----------------------------

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

async fn authenticate(
    bot: Bot,
    msg: Message,
    command: Command,
    db: Arc<SqlitePool>,
) -> HandlerResult {
    let Command::Authenticate(token, name) = command else {
        return Ok(()); // Cannot happen because of the dptree::case guard
    };

    if token == config().admin_token {
        let id = msg.chat.id.to_string();
        sqlx::query!(
            r#"INSERT INTO admins(telegram_id, "name") VALUES($1, $2)"#,
            id,
            name
        )
        .execute(db.as_ref())
        .await?;
        bot.send_message(msg.chat.id, "Authentication successful !")
            .await?;
    } else {
        bot.send_message(msg.chat.id, "Token is incorrect").await?;
    }

    Ok(())
}

async fn admin_list(bot: Bot, msg: Message, db: Arc<SqlitePool>) -> HandlerResult {
    let admins = sqlx::query!(r#"SELECT "name" FROM admins"#)
        .fetch_all(db.as_ref())
        .await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "Current admin(s):\n{}",
            admins
                .into_iter()
                .map(|r| format!(" - {}", r.name))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )
    .await?;

    Ok(())
}

async fn admin_remove(
    bot: Bot,
    msg: Message,
    command: Command,
    db: Arc<SqlitePool>,
) -> HandlerResult {
    let Command::AdminRemove(target) = command else {
        return Ok(()); // Cannot happen because of the guard
    };

    let mut tx = db.begin().await?;

    if sqlx::query!(
        "SELECT COUNT(*) AS count FROM admins WHERE name = $1",
        target
    )
    .fetch_one(tx.as_mut())
    .await?
    .count
        == 0
    {
        bot.send_message(msg.chat.id, format!("{} is not an admin", target))
            .await?;
        return Ok(());
    }

    sqlx::query!("DELETE FROM admins WHERE name = $1", target)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;

    bot.send_message(
        msg.chat.id,
        format!("{} successfully removed from admins", target),
    )
    .await?;

    Ok(())
}

async fn authorize(bot: Bot, msg: Message, command: String, db: Arc<SqlitePool>) -> HandlerResult {
    let mut tx = db.begin().await?;

    let chat_id_str = msg.chat.id.to_string();
    let already_authorized = sqlx::query!(
        r#"SELECT COUNT(*) AS count FROM authorizations WHERE chat_id = $1 AND command = $2"#,
        chat_id_str,
        command
    )
    .fetch_one(tx.as_mut())
    .await?;

    if already_authorized.count == 0 {
        sqlx::query!(
            r#"INSERT INTO authorizations(command, chat_id) VALUES($1, $2)"#,
            command,
            chat_id_str
        )
        .execute(tx.as_mut())
        .await?;
    }

    tx.commit().await?;

    bot.send_message(
        msg.chat.id,
        format!("Ce groupe peut désormais utiliser la commande /{}", command),
    )
    .await?;
    Ok(())
}

async fn unauthorize(
    bot: Bot,
    msg: Message,
    command: String,
    db: Arc<SqlitePool>,
) -> HandlerResult {
    let mut tx = db.begin().await?;

    let chat_id_str = msg.chat.id.to_string();
    let already_authorized = sqlx::query!(
        r#"SELECT COUNT(*) AS count FROM authorizations WHERE chat_id = $1 AND command = $2"#,
        chat_id_str,
        command
    )
    .fetch_one(tx.as_mut())
    .await?;

    if already_authorized.count > 0 {
        sqlx::query!(
            r#"DELETE FROM authorizations WHERE command = $1 AND chat_id = $2"#,
            command,
            chat_id_str
        )
        .execute(tx.as_mut())
        .await?;
    }

    tx.commit().await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "Ce groupe ne peut désormais plus utiliser la commande /{}",
            command
        ),
    )
    .await?;
    Ok(())
}

mod poll {
    use crate::commands::POLL_MAX_OPTIONS_COUNT;
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

            // Splits the committee to have only 10 answers possible.
            let mut poll = config().committee.clone();
            poll.retain(|s| -> bool { *s != target }); // filter the target from options
            poll.shuffle(&mut thread_rng()); // shuffle the options
            let index = thread_rng().gen_range(0..(POLL_MAX_OPTIONS_COUNT - 1)); // generate a valid index to insert target back
            poll.insert(index as usize, target.clone()); // insert target back in options
            let poll = poll.split_at(POLL_MAX_OPTIONS_COUNT as usize).0.to_vec(); // split options to have only 10 options

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

            log::debug!("Resetting dialogue status");
            dialogue.update(PollState::Start).await?;
        }

        Ok(())
    }
}
