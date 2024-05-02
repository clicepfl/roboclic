# Roboclic V2

The Telegram bot of the [CLIC](https://clic.epfl.ch), cleaned up and in Rust.

The available commands are:

- `/help`: Displays a help message.
- `/authenticate <token> <name>`: Authenticate as an admin user using the `ADMIN_TOKEN` provided in the environment variables and a name (can be any).
- Group restricted commands:
  - `/bureau`: Creates a poll querying who is at the desk (in INN132).
  - `/poll`: Creates a quiz where you need to find the committee behind a quote.
  - `/stats`: Display the stats of the committee (number of polls).
- Admin restricted commands:
  - `/adminlist`: List the admins.
  - `/adminremove <name>`: Remove an admin.
  - `/authorize <command>`: Authorize the current chat to use the given command (must be one of the command from the list above).
  - `/unauthorize <command>`: Unauthorize the current chat to use the given command (must be one of the command from the list above).

## Configuration

### Environment

- `BOT_TOKEN`: The token provided by [@BotFather](https://t.me/BotFather) to authenticate the bot in API calls.
- `ADMIN_TOKEN`: The token used to authenticate admin users.
- `DATA_DIR`: The directory where the bot will read/write data
- `DATABASE_URL` (optional): The url of the SQLite database. Defaults to `sqlite://${DATA_DIR}/db.sqlite`.
- `DIRECTUS_URL`: Base url of the Directus instance used.
- `DIRECTUS_TOKEN`: Token for Directus RoboCLIC user.

## Deployment

The bot can be run either using [cargo](https://doc.rust-lang.org/cargo/), or the provided Docker image.

The latter is preferred, since it allows off-the-shelf use. The configuration required is the same as specified above, the config file can directly be mounted in the container.

## References

- Language: [Rust](https://rust-lang.org)
- Telegram bot framework: [Teloxide](https://github.com/teloxide/teloxide/tree/master)
- Telegram API: [Telegram](https://core.telegram.org/bots)
- Directus API: [Directus](https://docs.directus.io/reference/introduction.html)
- Deployment methods: [Docker](https://docker.com)