use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::components::configurable_server::show;
use crate::config;
use crate::config::Config;
use crate::config::ConfigBuilder;
use crate::utils::interaction_reply;
use crate::utils::interaction_reply_ephemeral;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    debug!(command_options = ?command.data.options, "command options");

    let result: Result<Option<Config>, surrealdb::Error> = DB
        .select(("guild_config", command.guild_id.unwrap().to_string()))
        .await;

    match result {
        Ok(response) => match response {
            Some(c) => return interaction_reply(command, ctx.clone(), format!(":warning: This server is already configured with: {:?}\n Try using `/config_update` to change the config", c)).await,

            None => {}
        },
        Err(e) => return interaction_reply(command, ctx.clone(), format!("Database error: {}", e)).await,
    };

    let config = match Config::from_builder(ConfigBuilder::build(command)) {
        Some(c) => c,
        None => {
            return interaction_reply_ephemeral(
                command,
                ctx,
                ":x: Error building config, please ensure all fields are present!",
            )
            .await;
        }
    };

    debug!(config = ?config, "created config struct");

    let created: Result<Option<Config>, surrealdb::Error> = DB
        .create(("guild_config", config.guild_id.to_string()))
        .content(config)
        .await;

    match created {
        Ok(response) => match response {
            Some(c) => {
                show(&ctx, &command.channel_id, &c).await?;
                interaction_reply_ephemeral(command, ctx, ":white_check_mark: Configuration added successfully".to_string()).await
            }
            None => {
                warn!("Error adding configuration");
                interaction_reply_ephemeral(command, ctx.clone(), ":x: Error adding configuration".to_string()).await
            }
        },
        Err(e) => {
            error!(error = %e, "database error");
            interaction_reply_ephemeral(command, ctx.clone(), format!(":x: Database error: {}", e)).await
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    config::register_options(
        command
            .name("configure")
            .description("Configure options for SurrealBot")
            .default_member_permissions(Permissions::MANAGE_CHANNELS),
        true,
    )
}
