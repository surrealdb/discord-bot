use serenity::{
    model::{
        prelude::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
            PermissionOverwrite, PermissionOverwriteType,
        },
        Permissions,
    },
    prelude::Context,
};

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

pub fn read_view_perms(kind: PermissionOverwriteType) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            .union(Permissions::SEND_MESSAGES)
            .union(Permissions::READ_MESSAGE_HISTORY),
        deny: Permissions::empty(),
        kind: kind,
    }
}
