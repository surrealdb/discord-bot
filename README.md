# discord-bot

This is a discord bot that allows you to start a SurrealDB instance associated with a thread or channel and query it.

To run the bot you can either clone it and use
```
cargo run --release
```
(note your discord token must be supplied in the environment variables or a .env file) \
or use the docker container 
```
docker run -d -e "DISCORD_TOKEN=YOUR_TOKEN_HERE" surrealdb/discord-bot
```

# Discord commands

## Admin commands

### /configure
This command is used to initially configure the bot and must be used before any other functionality will work. \
It takes the following (mandatory) arguments
- active - the category for active channels
- archive - the category for archived channels
- ttl - the duration (in seconds) after which a channel will be archived
- pretty - whether to use pretty printing
- json - whether to format output as JSON (SurrealQL is the alternative)

### /config_update
This command takes the same arguments as the /configure command but optionally, and will update the config for the server with those options.

### /clean
This command runs the clean_channel function on the channel in which it is run, dropping the associated database instance, notifying the channel and archiving it.

### /clean_all
This command runs clean_channel on all current database instances.
PLEASE NOTE: this applies even across servers, and should only be used immediately before shutting the bot down.

## User commands

### /create
This creates a channel with an associated database instance. By default, all messages in the channel will be sent to the database, unless they are comments. \
Optional arguments:
- premade - choose from premade datasets to load into the instance
- file - upload a .surql file to import into the database

### create_db_thread
This is not a slash command but can be accessed by right-clicking on a message looking under apps and selecting create_db_thread, it will create a thread associated with that message. By default, all messages in the channel will be sent to the database, unless they are comments.

### /load
This command loads data into the instance associated with the channel in which it is used.

### /query (/q)
This command queries the database instance associated with the channel. This command is useful when a conversation is happening in a channel with an associated instance, the channel can be configured with /configure_channel so normal messages aren't sent to the database. It also means that the up arrow behaves in a useful manner for rapidly iterating on queries.

### /configure_channel
This command allows you to override the configuration for a channel.
- pretty - whether to use pretty printing
- json - whether to format output as JSON (SurrealQL is the alternative)
- require_query - whether the /query command is required, if it's false 