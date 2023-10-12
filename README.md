# Roboclic V2

The Telegram bot of the [CLIC](https://clic.epfl.ch), cleaned up and in Rust.

The available commands are:

- `/bureau`: Creates a poll querying who is at the desk (in INN132)
- `/poll`: Creates a quiz where you need to find the committee behind a quote
- `/help`: Displays a help message

## Setup

The bot is configured through a combination of two ways: environment variables, and a config file in json. Environment variables holds sensitive data, like the bot's token, while the file contains basic informations, like the list of the committee, or access control settings.

Any changes of the configuration requires a restart.

### Environment

- `BOT_TOKEN`: The token provided by [@BotFather](https://t.me/BotFather) to authenticate the bot in API calls.
- `CONFIG_FILE`: The location of the config file (see below)

### Configuration file

Written in JSON. Contains a dictionnary with the following entries:

- `"committee": Vec<String>`: An array of string of the names of each members of the committee.
- `"access_control": HashMap<String, Vec<i64>>`: A mapping specifying which chat can trigger which command. The keys are the commands in lowercase (see above), and the value a list of authorized chat ids.

  **Important**: An empty list means that no chat is allowed to trigger a command, while an absent key allows every chat to use it.

Example:

```json
{
  "committee": ["Committee 1", "Committee 2", "Committee 3", "Committee 4"],
  "access_control": {
    "bureau": [1234567890],
    "poll": [9876543210]
  }
}
```

## Deployment

The bot can be run either using [cargo](https://doc.rust-lang.org/cargo/), or the provided Docker image.

The latter is preferred, since it allows off-the-shelf use. The configuration required is the same as specified above, the config file can directly be mounted in the container.

## References:

- Language: [Rust](https://rust-lang.org)
- Telegram bot framework: [Teloxide](https://github.com/teloxide/teloxide/tree/master)
- Telegram API: [Telegram](https://core.telegram.org/bots)
- Deployment methods: [Docker](https://docker.com)
