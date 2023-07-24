use std::path::Path;

use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;
use tokio::fs;

use crate::utils::interaction_followup;
use crate::utils::interaction_reply;
use crate::utils::interaction_reply_edit;
use crate::utils::interaction_reply_ephemeral;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    println!("{:?}", command.data.options);

    let conn = match DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        Some(c) => c.clone(),
        None => {
            interaction_reply_ephemeral(
                command,
                ctx,
                "No database instance found for this channel",
            )
            .await?;
            return Ok(());
        }
    };
    interaction_reply(command, ctx.clone(), "Exporting database").await?;

    fs::create_dir("tmp").await.ok();
    let path = format!("tmp/{}.surql", command.id.as_u64());

    match conn.db.export(&path).await {
        Ok(_) => {
            command
                .create_followup_message(ctx, |message| {
                    message
                        .content("Database exported:")
                        .add_file(Path::new(&path))
                })
                .await?;

            fs::remove_file(path).await?;
        }
        Err(why) => {
            interaction_reply_edit(command, ctx, format!("Database export failed: {why}")).await?
        }
    };
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("export")
        .description("Export the database contents to a surql file")
}
