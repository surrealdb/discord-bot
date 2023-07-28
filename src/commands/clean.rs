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
            ":warning: There is no database instance currently associated with this channel",
        )
        .await?;
        return Ok(());
    }

    let channel = match command.channel_id.to_channel(&ctx).await.unwrap() {
        Channel::Guild(c) => c,
        _ => {
            interaction_reply_ephemeral(command, ctx, ":warning: This command only works in guild channels")
                .await?;
            return Ok(());
        }
    };
    interaction_reply_ephemeral(command, ctx.clone(), ":white_check_mark: This channel should now be cleaned").await?;

    clean_channel(channel, &ctx).await;

    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("clean")
        .description("Cleans the current channel!")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
}
