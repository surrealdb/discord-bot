use std::cmp::Ordering;
use std::path::Path;

use memorable_wordlist::kebab_case;
use serenity::model::prelude::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::{AttachmentType, Guild, PermissionOverwrite, UserId};
use serenity::model::Permissions;
use serenity::prelude::Context;
use serenity::{builder::CreateApplicationCommand, model::prelude::ChannelType};
use tracing::Instrument;

use crate::{premade, utils::*};

use crate::config::Config;
use crate::utils::{interaction_reply, interaction_reply_edit, interaction_reply_ephemeral};
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return interaction_reply_ephemeral(command, ctx, ":warning: No config found for this server, please ask an administrator to configure the bot".to_string()).await
                    }
                }
                Err(e) => return interaction_reply_ephemeral(command, ctx, format!(":x: Database error: {}", e)).await,
            };

            let guild = Guild::get(&ctx, id).await.unwrap();

            let everyone = guild.role_by_name("@everyone");

            let perms = vec![
                PermissionOverwrite {
                    allow: Permissions::empty(),
                    deny: Permissions::VIEW_CHANNEL,
                    kind: serenity::model::prelude::PermissionOverwriteType::Role(
                        everyone.unwrap().id,
                    ),
                },
                PermissionOverwrite {
                    allow: Permissions::VIEW_CHANNEL
                        .union(Permissions::SEND_MESSAGES)
                        .union(Permissions::READ_MESSAGE_HISTORY),
                    deny: Permissions::empty(),
                    kind: serenity::model::prelude::PermissionOverwriteType::Member(UserId(
                        command.application_id.as_u64().clone(),
                    )),
                },
                PermissionOverwrite {
                    allow: Permissions::VIEW_CHANNEL
                        .union(Permissions::SEND_MESSAGES)
                        .union(Permissions::READ_MESSAGE_HISTORY),
                    deny: Permissions::empty(),
                    kind: serenity::model::prelude::PermissionOverwriteType::Member(
                        command.user.id,
                    ),
                },
            ];

            let channel = guild
                .create_channel(&ctx, |c| {
                    c.name(kebab_case(40))
                        .kind(ChannelType::Text)
                        .category(config.active_channel)
                        .permissions(perms)
                })
                .await
                .unwrap();
            let db = create_db_instance(&config).await?;

            match command.data.options.len().cmp(&1) {
                Ordering::Greater => {
                    interaction_reply_ephemeral(command, ctx, ":information_source: Please only supply one argument (you can use the up arrow to edit the previous command)").await?;
                    return Ok(());
                }
                Ordering::Equal => {
                    let op_option = command.data.options[0].clone();
                    match op_option.kind {
                        CommandOptionType::String => {
                            match op_option.value.unwrap().as_str().unwrap() {
                                "surreal_deal_mini" => {
                                    interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance! Head over to <#{}> while the dataset is currently being loaded. Once you receive a confirmation, you can start to query against the Surreal deal (mini) dataset.", channel.id.as_u64())).await?;
                                    let db = db.clone();
                                    let (channel, ctx, command) =
                                        (channel.clone(), ctx.clone(), command.clone());
                                    tokio::spawn(async move {
                                        channel.say(&ctx, format!("## :information_source: This channel is now connected to a SurrealDB instance which is loading data, it will be ready to query soon!\n## _Please note this channel will expire after {:#?} of inactivity._", config.ttl)).await.unwrap();
                                        db.import("premade/surreal_deal_mini.surql").await.unwrap();
                                        channel.say(&ctx, format!("<@{}> This channel is now connected to a SurrealDB instance with the Surreal deal (mini) dataset, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                        channel
                                            .send_files(
                                                ctx,
                                                [AttachmentType::Path(&Path::new(
                                                    "premade/surreal_deal.png",
                                                ))],
                                                |m| m.content("schema:"),
                                            )
                                            .await
                                            .unwrap();
                                    }.instrument(tracing::Span::current()));
                                }
                                "surreal_deal" => {
                                    interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance! Head over to <#{}> while the dataset is currently being loaded. Once you receive a confirmation, you can start to query against the Surreal deal dataset.", channel.id.as_u64())).await?;
                                    let db = db.clone();
                                    let (channel, ctx, command) =
                                        (channel.clone(), ctx.clone(), command.clone());
                                    tokio::spawn(async move {
                                        channel.say(&ctx, format!("## :information_source: This channel is now connected to a SurrealDB instance which is loading data, it will be ready to query soon!\n## _Please note this channel will expire after {:#?} of inactivity._", config.ttl)).await.unwrap();
                                        db.import("premade/surreal_deal.surql").await.unwrap();
                                        channel.say(&ctx, format!("<@{}> This channel is now connected to a SurrealDB instance with the Surreal deal dataset, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                        channel
                                            .send_files(
                                                ctx,
                                                [AttachmentType::Path(&Path::new(
                                                    "premade/surreal_deal.png",
                                                ))],
                                                |m| m.content("schema:"),
                                            )
                                            .await
                                            .unwrap();
                                    }.instrument(tracing::Span::current()));
                                }
                                dataset => {
                                    warn!(dataset, "Unknown dataset was requested");
                                    interaction_reply_ephemeral(
                                        command,
                                        ctx,
                                        "Cannot find requested dataset",
                                    )
                                    .await?;
                                    return Ok(());
                                }
                            }
                        }
                        CommandOptionType::Attachment => {
                            if let Some(CommandDataOptionValue::Attachment(attachment)) =
                                op_option.resolved
                            {
                                interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance! Head over to <#{}> while your file is now being uploaded. Once you receive a confirmation, you can start querying against the imported dataset.", channel.id.as_u64())).await?;
                                match attachment.download().await {
                                    Ok(data) => {
                                        interaction_reply_edit(command, ctx.clone(), format!(":information_source: You now have your own database instance! hHead over to <#{}> while your file is now being imported. Once you receive a confirmation, you can start querying against the imported dataset.", channel.id.as_u64())).await?;

                                        let db = db.clone();
                                        let (channel, ctx, command) =
                                            (channel.clone(), ctx.clone(), command.clone());
                                        tokio::spawn(async move {
                                            channel.say(&ctx, format!("## :information_source: This channel is now connected to a SurrealDB instance which is loading data, it will be ready to query soon!\n## _Please note this channel will expire after {:#?} of inactivity._", config.ttl)).await.unwrap();
                                            if let Err(why) = db
                                                .query(String::from_utf8_lossy(&data).into_owned())
                                                .await
                                            {
                                                interaction_reply_edit(
                                                    &command,
                                                    ctx.clone(),
                                                    format!(":x: Error importing from file, please ensure that files are valid SurrealQL: {}", why),
                                                )
                                                .await
                                                .ok();
                                                channel.say(&ctx, format!(":x: Error loading data, channel will be deleted: {why}")).await.ok();
                                                channel.delete(ctx).await.ok();
                                                return;
                                            }
                                            channel.say(&ctx, format!("<@{}> This channel is now connected to a SurrealDB instance with your dataset, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                            interaction_reply_edit(
                                                &command,
                                                ctx,
                                                format!(":information_source: You now have your own database instance! Head over to <#{}> to start writing SurrealQL to query your data!", channel.id.as_u64()),
                                            )
                                            .await
                                            .unwrap();
                                        }.instrument(tracing::Span::current()));
                                    }
                                    Err(why) => {
                                        interaction_reply_edit(
                                            command,
                                            ctx,
                                            format!(":x: Error with attachment: {}", why),
                                        )
                                        .await?;
                                        return Ok(());
                                    }
                                }
                            } else {
                                interaction_reply_edit(command, ctx, ":x: Error with attachment")
                                    .await?;
                                return Ok(());
                            }
                        }
                        _ => {
                            interaction_reply_ephemeral(command, ctx, ":x: Unsupported option type")
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Ordering::Less => {
                    channel.say(&ctx, format!(":information_source: This channel is now connected to a SurrealDB instance, try writing some SurrealQL! \n_Please note this channel will expire after {:#?} of inactivity_", config.ttl)).await?;
                    interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance, head over to <#{}> to start writing SurrealQL!", channel.id.as_u64())).await?;
                }
            };

            register_db(
                ctx,
                db,
                channel,
                config,
                crate::ConnType::EphemeralChannel,
                false,
            )
            .await?;
            return Ok(());
        }
        None => {
            return interaction_reply(
                command,
                ctx,
                ":warning: Direct messages are not currently supported".to_string(),
            )
            .await;
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("create")
        .description("Creates a channel with a SurrealDB instance")
        .create_option(premade::register)
        .create_option(|option| {
            option
                .name("file")
                .description("A SurrealQL file to load into the database instance")
                .kind(CommandOptionType::Attachment)
                .required(false)
        })
}
