use std::env;
use std::path::Path;

use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::*;
use tokio::fs;

use crate::utils;
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

    let base_path = match env::var("TEMP_DIR_PATH") {
        Ok(p) => p,
        Err(_) => {
            fs::create_dir("tmp").await.ok();
            "tmp/".to_string()
        }
    };
    let path = format!("{base_path}{}.surql", command.id.as_u64());

    match conn.db.export(&path).await {
        Ok(_) => {
            match fs::metadata(&path).await {
                Ok(metadata) => {
                    if metadata.len() < utils::MAX_FILE_SIZE as u64 {
                        command
                            .create_followup_message(ctx, |message| {
                                message
                                    .content(":white_check_mark: Database exported:")
                                    .add_file(Path::new(&path))
                            })
                            .await?;
                    } else {
                        interaction_reply_edit(
                            command,
                            ctx,
                            ":x: Your database is too powerful, (the export is too large to send)",
                        )
                        .await?;
                    }
                }
                Err(_) => {
                    command
                        .create_followup_message(&ctx, |m| m.content(":x: Error in export process"))
                        .await?;
                }
            }

            fs::remove_file(path).await?;
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
