use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;

use crate::utils::ephemeral_interaction;
use crate::utils::CmdError;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if DBCONNS
        .lock()
        .await
        .get(command.channel_id.as_u64())
        .is_none()
    {
        return CmdError::NoSession.reply(&ctx, command).await;
    }

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
        ephemeral_interaction(
            &ctx,
            command,
            "Sharing channel",
            "User added to channel",
            Some(true),
        )
        .await?;
    } else {
        ephemeral_interaction(
            &ctx,
            command,
            "Already public",
            "Could not add user to channel as it's already a public thread.",
            Some(false),
        )
        .await?;
    }

    command
        .channel_id
        .say(
            &ctx,
            format!(
                ":white_check_mark: <@{}> can now view this channel and write SurrealQL",
                user_id
            ),
        )
        .await?;

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
