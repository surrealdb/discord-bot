use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;

use crate::utils::{ephemeral_interaction, CmdError};
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let conn = match DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        Some(c) => c.clone(),
        None => {
            CmdError::NoSession.reply(&ctx, command).await?;
            return Ok(());
        }
    };

    ephemeral_interaction(
        &ctx,
        command,
        "Exporting database",
        "This may take a while",
        None,
    )
    .await?;

    match conn.export_to_attachment().await {
        Ok(Some(attachment)) => {
            command.create_interaction_response(&ctx, |r| {
                r.interaction_response_data(|d| {
                    d.embed(|e| {
                        e.title("Exported successfully").description("Find the exported .surql file below.\nYou can either use `/load` and load a new session with it, or use it locally with `surreal import` CLI.").color(0x00ff00)
                    }).add_file(attachment)
                })
            }).await?;
        }
        Ok(None) => {
            CmdError::ExportTooLarge.reply(&ctx, command).await?;
        }
        Err(err) => {
            CmdError::ExportFailed(err).reply(&ctx, command).await?;
        }
    };
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("export")
        .description("Export the database contents to a surql file")
}
