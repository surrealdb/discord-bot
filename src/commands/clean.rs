use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;

use serenity::prelude::*;

use crate::utils::clean_channel;
use crate::utils::interaction_reply_ephemeral;
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
        interaction_reply_ephemeral(
            command,
            ctx,
            "There is no database instance currently associated with this channel",
        )
        .await?;
        return Ok(());
    }

    let channel = match command.channel_id.to_channel(&ctx).await.unwrap() {
        Channel::Guild(c) => c,
        _ => {
            interaction_reply_ephemeral(command, ctx, "Command only works in guild channels")
                .await?;
            return Ok(());
        }
    };
    clean_channel(channel, &ctx).await;

    interaction_reply_ephemeral(command, ctx, "This channel should now be cleaned").await
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("clean")
        .description("cleans the current channel!")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
}
