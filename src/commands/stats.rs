use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::stats::collect_stats;
use crate::utils::ephemeral_interaction;
use crate::utils::CmdError;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    info!(
        "stats called from channel: {:?}\n{:?}",
        command.channel_id,
        ctx.http.get_channel(command.channel_id.0).await?
    );
    match collect_stats(ctx.http.clone()).await {
        Ok(s) => {
            info!("got stats: {s:?}");
            ephemeral_interaction(
                ctx.http,
                command,
                "Statistics generated",
                format!("got stats: {s:?}"),
                None,
            )
            .await?
        }
        Err(e) => return CmdError::Stats(e.to_string()).reply(&ctx, command).await,
    }

    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("stats")
        .description("Generate statistics about the server")
        .default_member_permissions(Permissions::MANAGE_CHANNELS)
}
