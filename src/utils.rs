use serenity::{
    model::{
        prelude::{
            application_command::ApplicationCommandInteraction, GuildChannel,
            InteractionResponseType, PermissionOverwrite, PermissionOverwriteType,
        },
        Permissions,
    },
    prelude::Context,
};

use crate::{db_utils::get_config, DBCONNS};

pub async fn interaction_reply(
    command: &ApplicationCommandInteraction,
    ctx: Context,
    content: impl ToString,
) -> Result<(), anyhow::Error> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(content))
        })
        .await?;
    Ok(())
}

pub async fn interaction_reply_ephemeral(
    command: &ApplicationCommandInteraction,
    ctx: Context,
    content: impl ToString,
) -> Result<(), anyhow::Error> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(content).ephemeral(true))
        })
        .await?;
    Ok(())
}

pub async fn interaction_reply_edit(
    command: &ApplicationCommandInteraction,
    ctx: Context,
    content: impl ToString,
) -> Result<(), anyhow::Error> {
    command
        .edit_original_interaction_response(&ctx.http, |response| response.content(content))
        .await?;
    Ok(())
}

pub async fn interaction_followup(
    command: &ApplicationCommandInteraction,
    ctx: Context,
    content: impl ToString,
) -> Result<(), anyhow::Error> {
    command
        .create_followup_message(&ctx.http, |response| response.content(content))
        .await?;
    Ok(())
}

pub fn read_view_perms(kind: PermissionOverwriteType) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            .union(Permissions::SEND_MESSAGES)
            .union(Permissions::READ_MESSAGE_HISTORY),
        deny: Permissions::empty(),
        kind: kind,
    }
}

pub async fn clean_channel(mut channel: GuildChannel, ctx: &Context) {
    let _ = channel
        .say(
            &ctx,
            "This database instance has expired and is no longer functional",
        )
        .await;

    DBCONNS.lock().await.remove(channel.id.as_u64());

    let result = get_config(channel.guild_id).await;

    let response = match result {
        Ok(o) => o,
        Err(_) => return,
    };

    let config = match response {
        Some(c) => c,
        None => return,
    };

    let _ = channel
        .edit(ctx, |c| c.category(config.archive_channel))
        .await;
}
