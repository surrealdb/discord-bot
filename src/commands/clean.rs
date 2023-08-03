use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;

use serenity::prelude::*;

use crate::utils::clean_channel;
use crate::utils::ephemeral_interaction;
use crate::utils::CmdError;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if !DBCONNS
        .lock()
        .await
        .contains_key(command.channel_id.as_u64())
    {
        CmdError::NoSession.reply(&ctx, command).await?;
        return Ok(());
    }

    let channel = match command.channel_id.to_channel(&ctx).await.unwrap() {
        Channel::Guild(c) => c,
        _ => {
            CmdError::NoGuild.reply(&ctx, command).await?;
            return Ok(());
        }
    };

    ephemeral_interaction(
        &ctx,
        command,
        "Cleaned channel",
        "This channel should now be cleaned",
        Some(true),
    )
    .await?;

    clean_channel(channel, &ctx).await;

    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("clean")
        .description("Cleans the current channel!")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
}
