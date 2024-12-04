use crate::{commands::RESTRICTED_COMMANDS, config::config, HandlerResult};
use sqlx::SqlitePool;
use std::sync::Arc;
use teloxide::{requests::Requester, types::Message, Bot};

pub async fn authenticate(
    bot: Bot,
    msg: Message,
    (token, name): (String, String),
    db: Arc<SqlitePool>,
) -> HandlerResult {
    if token == config().admin_token {
        let id = msg.chat.id.to_string();
        sqlx::query!(
            r#"INSERT INTO admins(telegram_id, "name") VALUES($1, $2)"#,
            id,
            name
        )
        .execute(db.as_ref())
        .await?;
        bot.send_message(msg.chat.id, "Authentification réussie !")
            .await?;
    } else {
        bot.send_message(msg.chat.id, "Le token est incorrect")
            .await?;
    }

    Ok(())
}

pub async fn admin_list(bot: Bot, msg: Message, db: Arc<SqlitePool>) -> HandlerResult {
    let admins = sqlx::query!(r#"SELECT "name" FROM admins"#)
        .fetch_all(db.as_ref())
        .await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "Admin(s) actuel(s):\n{}",
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

pub async fn admin_remove(
    bot: Bot,
    msg: Message,
    name: String,
    db: Arc<SqlitePool>,
) -> HandlerResult {
    let mut tx = db.begin().await?;

    if sqlx::query!("SELECT COUNT(*) AS count FROM admins WHERE name = $1", name)
        .fetch_one(tx.as_mut())
        .await?
        .count
        == 0
    {
        bot.send_message(msg.chat.id, format!("{} n'est pas admin", name))
            .await?;
        return Ok(());
    }

    sqlx::query!("DELETE FROM admins WHERE name = $1", name)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;

    bot.send_message(msg.chat.id, format!("{} a été retiré(e) des admins", name))
        .await?;

    Ok(())
}

pub async fn authorize(
    bot: Bot,
    msg: Message,
    command: String,
    db: Arc<SqlitePool>,
) -> HandlerResult {
    let mut tx = db.begin().await?;

    let chat_id_str = msg.chat.id.to_string();

    if !RESTRICTED_COMMANDS.iter().any(|c| c.shortand() == command) {
        bot.send_message(msg.chat.id, "Cette commande n'existe pas")
            .await?;
        return Ok(());
    }

    let already_authorized = sqlx::query!(
        r#"SELECT COUNT(*) AS count FROM authorizations WHERE chat_id = $1 AND command = $2"#,
        chat_id_str,
        command
    )
    .fetch_one(tx.as_mut())
    .await?;

    if already_authorized.count > 0 {
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

pub async fn unauthorize(
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

pub async fn authorizations(bot: Bot, msg: Message, db: Arc<SqlitePool>) -> HandlerResult {
    let chat_id_str = msg.chat.id.to_string();
    let authorizations = sqlx::query!(
        r#"SELECT command FROM authorizations WHERE chat_id = $1"#,
        chat_id_str
    )
    .fetch_all(db.as_ref())
    .await?;

    bot.send_message(
        msg.chat.id,
        if authorizations.is_empty() {
            "Ce groupe ne peut utiliser aucune commande".to_owned()
        } else {
            format!(
                "Ce groupe peut utiliser les commandes suivantes:\n{}",
                authorizations
                    .into_iter()
                    .map(|s| format!(" - {}", s.command))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        },
    )
    .await?;

    Ok(())
}
