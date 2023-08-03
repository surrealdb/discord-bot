use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::components::configurable_server::show;
use crate::config::{self, Config, ConfigBuilder};
use crate::utils::{ephemeral_interaction, CmdError};
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
        Ok(response) => {
            if response.is_some() {
                return CmdError::ExpectedNoSession.reply(&ctx, command).await;
            }
        }
        Err(e) => return CmdError::GetConfig(e).reply(&ctx, command).await,
    };

    let config = match Config::from_builder(ConfigBuilder::build(command)) {
        Some(c) => c,
        None => return CmdError::BuildConfig.reply(&ctx, command).await,
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
                ephemeral_interaction(
                    &ctx,
                    command,
                    "Config added",
                    "Configuration added successfully",
                    Some(true),
                )
                .await
            }
            None => {
                warn!("Error adding configuration");
                ephemeral_interaction(
                    &ctx,
                    command,
                    "Config not updated",
                    "Error updating configuration",
                    Some(false),
                )
                .await
            }
        },
        Err(e) => {
            error!(error = %e, "database error");
            CmdError::UpdateConfig(e).reply(&ctx, command).await
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
