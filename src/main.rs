use sqlx::{migrate::MigrateDatabase, SqlitePool};
use teloxide::{
    dispatching::dialogue::{self, InMemStorage},
    prelude::*,
    utils::command::BotCommands,
};

use crate::commands::{
    command_callback_query_handler, command_message_handler, Command, PollState,
};

mod commands;
mod config;

pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn init_db() -> SqlitePool {
    if !sqlx::Sqlite::database_exists(&config::config().database_url)
        .await
        .unwrap()
    {
        sqlx::Sqlite::create_database(&config::config().database_url)
            .await
            .unwrap();
    }

    let database = SqlitePool::connect(&config::config().database_url)
        .await
        .unwrap();
    sqlx::migrate!().run(&database).await.unwrap();

    database
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    log::info!("Loading config files");
    config::config();
    let database = init_db().await;

    let bot = Bot::new(config::config().bot_token.clone());
    bot.set_my_commands(Command::bot_commands()).await.unwrap();

    log::info!("Initializing dispatchers");
    let message_handler = Update::filter_message().chain(command_message_handler());
    let callback_handler = Update::filter_callback_query().chain(command_callback_query_handler());

    let mut bot_dispatcher = Dispatcher::builder(
        bot,
        dialogue::enter::<Update, InMemStorage<commands::PollState>, commands::PollState, _>()
            .branch(message_handler)
            .branch(callback_handler),
    )
    .default_handler(|_| async move {})
    .error_handler(LoggingErrorHandler::with_custom_text(
        "An error has occurred in the dispatcher",
    ))
    .dependencies(dptree::deps![InMemStorage::<PollState>::new(), database])
    .enable_ctrlc_handler()
    .build();

    log::info!("Starting command bot");
    bot_dispatcher.dispatch().await;
}
