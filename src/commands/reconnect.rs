use std::sync::Arc;

use anyhow::anyhow;
use serenity::{
    builder::CreateApplicationCommand,
    futures::StreamExt,
    model::prelude::{application_command, Attachment, ChannelId},
    prelude::Context,
};
use tracing::Instrument;

use crate::{
    components::configurable_session::show,
    config::Config,
    utils::{
        create_db_instance, ephemeral_interaction, ephemeral_interaction_edit, register_db,
        CmdError, ToInteraction,
    },
    DB, DBCONNS,
};

pub async fn run(
    command: &application_command::ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    match command.guild_id {
        Some(guild_id) => {
            if let Some(_) = DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
                CmdError::ExpectedNoSession.reply(&ctx, command).await
            } else {
                let result: Result<Option<Config>, surrealdb::Error> =
                    DB.select(("guild_config", guild_id.to_string())).await;

                let config = match result {
                    Ok(response) => match response {
                        Some(c) => c,
                        None => return CmdError::NoConfig.reply(&ctx, command).await,
                    },
                    Err(e) => return CmdError::GetConfig(e).reply(&ctx, command).await,
                };

                ephemeral_interaction(
                    &ctx,
                    command,
                    "Looking for last export",
                    "I will now check the last 20 messages for .surql attachments.",
                    None,
                )
                .await?;
                tokio::spawn(
                    reconnect(
                        ctx,
                        Arc::new(command.clone()),
                        command.channel_id,
                        config,
                        20,
                    )
                    .in_current_span(),
                );
                Ok(())
            }
        }
        None => CmdError::NoGuild.reply(&ctx, command).await,
    }
}

async fn reconnect(
    ctx: Context,
    i: impl ToInteraction,
    channel_id: ChannelId,
    config: Config,
    limit: usize,
) -> Result<(), anyhow::Error> {
    match find_attachment(&ctx, &i, channel_id, limit).await {
        Some(att) => new_db_from_attachment(ctx.clone(), i, channel_id, config, att).await,
        None => {
            ephemeral_interaction_edit(
                &ctx,
                i,
                "No export found!",
                "Bot could not find any .surql attachments in the last 20 messages.",
                Some(false),
            )
            .await
        }
    }
}

pub async fn new_db_from_attachment(
    ctx: Context,
    i: impl ToInteraction,
    channel_id: ChannelId,
    config: Config,
    att: Attachment,
) -> Result<(), anyhow::Error> {
    let channel = channel_id
        .to_channel(&ctx)
        .await?
        .guild()
        .ok_or(anyhow!("Not in a guild"))?;

    match create_db_instance(&config).await {
        Ok(db) => {
            match register_db(
                ctx.clone(),
                db.clone(),
                channel.clone(),
                config.clone(),
                crate::ConnType::ConnectedChannel,
                true,
            )
            .await
            {
                Ok(conn) => {
                    ephemeral_interaction_edit(&ctx, i.clone(), "Session loading!", "Successfully created a new session, registered it with this channel and now loading your export.", None).await?;
                    if let Err(err) = conn.import_from_attachment(&ctx, i.clone(), &att).await {
                        error!(error = %err, "Error importing from attachment")
                    }
                    show(&ctx, &channel, conn.conn_type, &config).await
                }
                Err(e) => CmdError::RegisterDB(e).edit(&ctx, i).await,
            }
        }
        Err(err) => {
            error!(error = %err, "Error creating DB instance");
            CmdError::CreateDB(err).edit(&ctx, i).await
        }
    }
}

#[tracing::instrument(skip(ctx, i), fields(channel_id = %channel_id, limit = %limit))]
async fn find_attachment(
    ctx: &Context,
    i: &impl ToInteraction,
    channel_id: ChannelId,
    limit: usize,
) -> Option<Attachment> {
    let mut messages = channel_id.messages_iter(ctx).boxed();
    let mut total = 0;
    loop {
        total += 1;
        if total > limit {
            break None;
        }
        if let Some(message_result) = messages.next().await {
            match message_result {
                Ok(message) => match message.attachments.first() {
                    Some(att) => break Some(att.clone()),
                    None => continue,
                },
                Err(error) => {
                    error!(error = %error, "Error getting message");
                    ephemeral_interaction_edit(
                        ctx,
                        i.clone(),
                        "Failed to get message",
                        format!("Couldn't load a message:\n```rust\n{error}\n```"),
                        Some(false),
                    )
                    .await
                    .unwrap();
                    break None;
                }
            }
        } else {
            break None;
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("reconnect")
        .description("Recreates a SurrealDB instance using most recent export and associates it with the current channel")
}
