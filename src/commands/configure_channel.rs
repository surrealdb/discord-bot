use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;

use crate::utils::{interaction_reply, interaction_reply_ephemeral};
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let options = command.data.options.clone();

    if let Some(conn) = DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
        for option in options {
            match option.name.as_str() {
                "pretty" => conn.pretty = option.value.clone().unwrap().as_bool().unwrap(),
                "json" => conn.json = option.value.clone().unwrap().as_bool().unwrap(),
                "require_query" => {
                    conn.require_query = option.value.clone().unwrap().as_bool().unwrap()
                }
                _ => {}
            }
        }
        interaction_reply(
            command,
            ctx,
            format!(
                ":information_source: **This channel's configuration was updated** \nPretty printing: `{}` \nJSON: `{}` \nRequire query: `{}`",
                conn.pretty, conn.json, conn.require_query
            ),
        )
        .await?;
    } else {
        interaction_reply_ephemeral(
            command,
            ctx,
            ":warning: There is no database instance currently associated with this channel",
        )
        .await?;
    }

    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("configure_channel")
        .description("Update configuration options on this channel for SurrealBot")
        .create_option(|option| {
            option
                .name("pretty")
                .description("Whether or not to pretty print responses")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
        .create_option(|option| {
            option
                .name("json")
                .description("Whether to format output as JSON, the alternative is SurrealQL")
                .kind(CommandOptionType::Boolean)
                .default_option(false)
                .required(false)
        })
        .create_option(|option| {
            option
                .name("require_query")
                .description("Whether the /query command is required to send queries to SurrealDB")
                .kind(CommandOptionType::Boolean)
                .default_option(false)
                .required(false)
        })
}
