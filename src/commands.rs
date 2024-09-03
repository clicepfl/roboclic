use std::sync::Arc;

use sqlx::SqlitePool;
use teloxide::{
    dispatching::DpHandlerDescription,
    prelude::*,
    types::{Message, MessageCommon, MessageKind},
    utils::command::BotCommands,
    Bot,
};

use crate::{
    cmd_authentication::{
        admin_list, admin_remove, authenticate, authorizations, authorize, unauthorize,
    },
    cmd_bureau::bureau,
    cmd_poll::{choose_target, set_quote, start_poll_dialogue, stats, PollState},
    HandlerResult,
};

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
                        .branch(dptree::case![Command::Poll].endpoint(start_poll_dialogue))
                        .branch(dptree::case![Command::Stats].endpoint(stats)),
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
                            )
                            .branch(
                                dptree::case![Command::Authorizations].endpoint(authorizations),
                            ),
                    ),
                ),
        )
        .branch(dptree::case![PollState::SetQuote { message_id, target }].endpoint(set_quote))
}

pub fn command_callback_query_handler(
) -> Endpoint<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    dptree::case![PollState::ChooseTarget { message_id }].endpoint(choose_target)
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
        let Some(user) = msg.from else {
            return false;
        };

        let id = user.id.to_string();
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
    #[command(description = "(Admin) Liste les commandes que ce groupe peut utiliser")]
    Authorizations,
    #[command(description = "(Admin) Affiche les stats des membres du comité")]
    Stats,
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
            Self::Authorizations => "authorizations",
            Self::Stats => "stats",
        }
    }
}

// ---------------------------- COMMAND ENDPOINTS -----------------------------

async fn help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}
