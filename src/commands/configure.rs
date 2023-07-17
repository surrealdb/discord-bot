use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

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
    println!("\n\n\n\n");
    println!("{:?}", command.data.options);

    let result: Result<Option<Config>, surrealdb::Error> = DB
        .select(("guild_config", command.guild_id.unwrap().to_string()))
        .await;

    match result {
        Ok(response) => match response {
            Some(c) => return interaction_reply(command, ctx.clone(), format!("This server is already configured with: {:?}\n Try using /configUpdate to change the config", c)).await,

            None => {}
        },
        Err(e) => return interaction_reply(command, ctx.clone(), format!("Database error: {}", e)).await,
    };

    assert_eq!(command.data.options[0].name, "active");
    assert_eq!(command.data.options[1].name, "archive");

    let config = match Config::from_builder(ConfigBuilder::build(command)) {
        Some(c) => c,
        None => {
            return interaction_reply_ephemeral(
                command,
                ctx,
                "Error building config, please ensure all fields are present",
            )
            .await;
        }
    };

    println!("created config struct");
    println!("{:?}", config);

    let created: Result<Option<Config>, surrealdb::Error> = DB
        .create(("guild_config", config.guild_id.to_string()))
        .content(config)
        .await;

    let msg = match created {
        Ok(response) => match response {
            Some(c) => {
                format!("This server is now configured with: {:?}", c)
            }

            None => "Error adding configuration".to_string(),
        },
        Err(e) => format!("Database error: {}", e),
    };
    interaction_reply(command, ctx.clone(), msg).await
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
