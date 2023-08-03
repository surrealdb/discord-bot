use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::components::configurable_server::show;
use crate::config;
use crate::config::Config;
use crate::config::ConfigBuilder;
use crate::utils::ephemeral_interaction;
use crate::utils::CmdError;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let result: Result<Option<Config>, surrealdb::Error> = DB
        .select(("guild_config", command.guild_id.unwrap().to_string()))
        .await;

    let mut config: Config = match result {
        Ok(response) => match response {
            Some(c) => c,

            None => return CmdError::NoSession.reply(&ctx, command).await,
        },
        Err(e) => return CmdError::GetConfig(e).reply(&ctx, command).await,
    };

    debug!(config = ?config, "existing config");

    let changes: ConfigBuilder = ConfigBuilder::build(command);
    config.merge(changes);

    debug!(config = ?config, "edited config");

    let updated: Result<Option<Config>, surrealdb::Error> = DB
        .update(("guild_config", config.guild_id.to_string()))
        .content(config)
        .await;

    match updated {
        Ok(response) => match response {
            Some(c) => {
                show(&ctx, &command.channel_id, &c).await?;
                ephemeral_interaction(
                    &ctx,
                    command,
                    "Config updated",
                    "Configuration updated successfully",
                    Some(true),
                )
                .await
            }

            None => {
                error!("Config update returned None");
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
            .name("config_update")
            .description("Update configuration options for SurrealBot in this server")
            .default_member_permissions(Permissions::MANAGE_CHANNELS),
        false,
    )
}
