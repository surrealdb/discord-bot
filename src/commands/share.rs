use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;

use crate::utils::interaction_reply;
use crate::utils::interaction_reply_ephemeral;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    println!("{:?}", command.data.options);

    if let None = DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        interaction_reply_ephemeral(
            command,
            ctx,
            ":information_source: Please use the `/share` command from an active SurrealQL channel",
        )
        .await?;
        return Ok(());
    }

    println!("{:?}", command.data.options[0]);
    let user_id = command.data.options[0]
        .value
        .clone()
        .unwrap()
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();

    if let Channel::Guild(channel) = command.channel_id.to_channel(&ctx).await? {
        command
            .channel_id
            .edit(&ctx, |c| {
                c.permissions(channel.permission_overwrites.clone().into_iter().chain([
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Member(UserId(user_id)),
                    },
                ]))
            })
            .await?;
    }

    interaction_reply(
        command,
        ctx,
        format!(
            ":white_check_mark: <@{}> can now view this channel and write SurrealQL",
            user_id
        ),
    )
    .await?;

    // interaction_reply(command, ctx.clone(), "not yet implemented".to_string()).await
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("share")
        .description("Share a SurrealQL channel with another user")
        .create_option(|option| {
            option
                .name("user")
                .description("User to share the channel with")
                .kind(CommandOptionType::User)
                .required(true)
        })
}
