use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::config;
use crate::config::Config;
use crate::config::ConfigBuilder;
use crate::utils::interaction_reply;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    debug!(command_options = ?command.data.options, "command options");

    let result: Result<Option<Config>, surrealdb::Error> = DB
        .select(("guild_config", command.guild_id.unwrap().to_string()))
        .await;

    let mut config: Config = match result {
        Ok(response) => match response {
            Some(c) => {c}

            None => return interaction_reply(command, ctx.clone(), ":warning: This server is not yet configured, use `/configure` to add initial configuration".to_string()).await,
        },
        Err(e) => return interaction_reply(command, ctx.clone(), format!("Database error: {}", e)).await,
    };

    debug!(config = ?config, "existing config");

    let changes: ConfigBuilder = ConfigBuilder::build(command);
    config.merge(changes);

    debug!(config = ?config, "edited config");

    let updated: Result<Option<Config>, surrealdb::Error> = DB
        .update(("guild_config", config.guild_id.to_string()))
        .content(config)
        .await;

    let msg = match updated {
        Ok(response) => match response {
            Some(c) => {
                format!(
                    ":white_check_mark: This server is now configured with: {:?}",
                    c
                )
            }

            None => {
                warn!("error updating configuration");
                ":x: Error updating configuration".to_string()
            }
        },
        Err(e) => {
            error!(error = %e, "database error");
            format!(":x: Database error: {}", e)
        }
    };
    interaction_reply(command, ctx.clone(), msg).await
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
