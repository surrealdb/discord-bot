use anyhow::bail;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::*;
use serenity::model::Permissions;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;

use crate::config::Config;
use crate::utils::interaction_reply;
use crate::utils::interaction_reply_ephemeral;
use crate::utils::read_view_perms;
use crate::DB;
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    println!("{:?}", command.data.options);

    // println!(
    //     "\n\n{:?}",
    //     command.data.options[0]
    //         .value
    //         .clone()
    //         .unwrap()
    //         .as_str()
    //         .unwrap()
    // );
    // println!(
    //     "\n\n{:?}",
    //     command.data.options[0].resolved.clone().unwrap()
    // );
    if let None = DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        interaction_reply_ephemeral(
            command,
            ctx,
            "Please use the /share command from an active SurrealQL channel",
        )
        .await?;
        return Ok(());
    }

    // if let application::interaction::application_command::CommandDataOptionValue::User(user, _) =
    //     command.data.options[0].resolved.clone().unwrap()
    // {
    //     println!("{:?}", user.id);
    //     command
    //         .channel_id
    //         .edit(ctx, |c| {
    //             c.permissions([read_view_perms(PermissionOverwriteType::Member(
    //                 user.id.clone(),
    //             ))])
    //         })
    //         .await?;
    // } else {
    //     bail!("Cant get userId from interaction")
    // }

    match DBCONNS.lock().await.get(command.channel_id.as_u64()) {
        Some(_) => {
            println!("{:?}", command.data.options[0]);
            let user_id = command.data.options[0]
                .value
                .clone()
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
            command
                .channel_id
                .edit(&ctx, |c| {
                    c.permissions([PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Member(UserId(user_id)),
                    }])
                })
                .await?;
            interaction_reply(
                command,
                ctx,
                format!(
                    "<@{}> can now view this channel and write SurrealQL",
                    user_id
                ),
            )
            .await?;
        }
        None => {
            interaction_reply_ephemeral(
                command,
                ctx,
                "Please use the /share command from an active SurrealQL channel",
            )
            .await?;
        }
    }

    // interaction_reply(command, ctx.clone(), "not yet implemented".to_string()).await
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("share")
        .description("Share a channel with another user")
        .create_option(|option| {
            option
                .name("user")
                .description("User to share the channel with")
                .kind(CommandOptionType::User)
                .required(true)
        })
}
