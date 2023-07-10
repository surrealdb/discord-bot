use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;

use crate::config::Config;
use crate::utils::interaction_reply;
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

    let config = Config {
        guild_id: command.guild_id.unwrap(),
        active_channel: ChannelId(
            command.data.options[0]
                .value
                .clone()
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
        ),
        archive_channel: ChannelId(
            command.data.options[1]
                .value
                .clone()
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
        ),
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
    command
        .name("configure")
        .description("Configure options for SurrealBot")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
        .create_option(|option| {
            option
                .name("active")
                .description("channel category for current database instances")
                .kind(CommandOptionType::Channel)
                .channel_types(&[ChannelType::Category])
                .required(true)
        })
        .create_option(|option| {
            option
                .name("archive")
                .description("channel category for archived database instances")
                .kind(CommandOptionType::Channel)
                .channel_types(&[ChannelType::Category])
                .required(true)
        })
}
