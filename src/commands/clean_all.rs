use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;

use serenity::prelude::*;
use tracing::Instrument;

use crate::utils::clean_channel;
use crate::utils::interaction_reply_ephemeral;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let map = (*DBCONNS.lock().await).clone();
    for id in map.keys() {
        let channel = match ChannelId(id.clone()).to_channel(&ctx).await.unwrap() {
            Channel::Guild(c) => c,
            _ => {
                interaction_reply_ephemeral(command, ctx, ":warning: This command only works in guild channels")
                    .await?;
                return Ok(());
            }
        };
        let (channel, ctx) = (channel.clone(), ctx.clone());
        tokio::spawn(async move { clean_channel(channel, &ctx).await }.instrument(tracing::Span::current()));
    }

    interaction_reply_ephemeral(command, ctx, ":white_check_mark: All channels should now be cleaned").await
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("clean_all")
        .description("Cleans all channels, this should only be used before a bot is shutdown!")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
}
