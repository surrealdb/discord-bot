use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use memorable_wordlist::kebab_case;
use serenity::prelude::Context;

use crate::utils::*;

use crate::config::Config;
use crate::utils::interaction_reply;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return interaction_reply_ephemeral(command, ctx, ":warning: No config found for this server, please ask an administrator to configure the bot".to_string()).await
                    }
                }
                Err(e) => return interaction_reply_ephemeral(command, ctx, format!("Database error: {}", e)).await,
            };

            let message = command.data.resolved.messages.keys().next().unwrap();

            let channel = command
                .channel_id
                .create_public_thread(&ctx, message, |t| t.name(kebab_case(40)))
                .await?;

            let db = create_db_instance(&config).await?;

            channel.say(&ctx, format!(":information_source: This public thread is now connected to a SurrealDB instance. Try writing some SurrealQL! \nIf you want, you can use `/load` to load a premade dataset or your own SurrealQL from a file. \n_Please note this channel will expire after {:#?} of inactivity._", config.ttl)).await?;
            interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance! Head over to <#{}> to start writing SurrealQL!", channel.id.as_u64())).await?;

            register_db(ctx, db, channel, config, crate::ConnType::Thread, true).await?;
            return Ok(());
        }
        None => {
            return interaction_reply(
                command,
                ctx,
                ":warning: Direct messages are not currently supported".to_string(),
            )
            .await;
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("create_db_thread")
        .kind(serenity::model::prelude::command::CommandType::Message)
}
