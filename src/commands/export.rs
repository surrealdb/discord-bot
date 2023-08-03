use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::utils::interaction_reply;
use crate::utils::interaction_reply_edit;
use crate::utils::interaction_reply_ephemeral;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let conn = match DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        Some(c) => c.clone(),
        None => {
            interaction_reply_ephemeral(
                command,
                ctx,
                ":x: No database instance found for this channel",
            )
            .await?;
            return Ok(());
        }
    };
    interaction_reply(
        command,
        ctx.clone(),
        ":information_source: Exporting database",
    )
    .await?;
    match conn.export_to_attachment().await {
        Ok(Some(reply_attachment)) => {
            command
                .create_followup_message(ctx, |message| {
                    message
                        .content(":white_check_mark: Database exported:")
                        .add_file(reply_attachment)
                })
                .await?;
        }
        Ok(None) => {
            interaction_reply_edit(
                command,
                ctx,
                ":x: Your database is too powerful, (the export is too large to send)",
            )
            .await?;
        }
        Err(why) => {
            interaction_reply_edit(command, ctx, format!(":x: Database export failed: {why}"))
                .await?
        }
    };
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("export")
        .description("Export the database contents to a surql file")
}
