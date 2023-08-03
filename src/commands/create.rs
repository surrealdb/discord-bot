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

use crate::components::configurable_session::show;
use crate::{premade, utils::*};

use crate::config::Config;
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
                Ok(response) => match response {
                    Some(c) => c,
                    None => return CmdError::NoConfig.reply(&ctx, command).await,
                },
                Err(e) => return CmdError::GetConfig(e).reply(&ctx, command).await,
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
                        *command.application_id.as_u64(),
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

            let config_clone = config.clone();

            match command.data.options.len().cmp(&1) {
                Ordering::Greater => {
                    CmdError::TooManyArguments(1, command.data.options.len())
                        .reply(&ctx, command)
                        .await?;
                    return Ok(());
                }
                Ordering::Equal => {
                    let op_option = command.data.options[0].clone();
                    match op_option.kind {
                        CommandOptionType::String => {
                            match op_option.value.unwrap().as_str().unwrap() {
                                "surreal_deal_mini" => {
                                    ephemeral_interaction(&ctx, command,
                                        "Database instance created, loading dataset...",
                                        format!("You now have your own database instance! Head over to <#{}> while the dataset is currently being loaded.\nOnce you receive a confirmation, you can start to query against the Surreal deal (mini) dataset.", channel.id.as_u64()),
                                        None,
                                    ).await?;
                                    let db = db.clone();
                                    let (channel, ctx, command) =
                                        (channel.clone(), ctx.clone(), command.clone());
                                    tokio::spawn(async move {
                                        show(&ctx, &channel, crate::ConnType::EphemeralChannel, &config_clone).await.unwrap();
                                        db.import("premade/surreal_deal_mini.surql").await.unwrap();
                                        channel.say(&ctx, format!("<@{}> Your instance now has Surreal deal (mini) dataset loaded, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                        channel
                                            .send_files(
                                                ctx,
                                                [AttachmentType::Path(Path::new(
                                                    "premade/surreal_deal.png",
                                                ))],
                                                |m| m.content("schema:"),
                                            )
                                            .await
                                            .unwrap();
                                    }.in_current_span());
                                }
                                "surreal_deal" => {
                                    ephemeral_interaction(&ctx, command,
                                        "Database instance created, loading dataset...",
                                        format!("You now have your own database instance! Head over to <#{}> while the dataset is currently being loaded.\nOnce you receive a confirmation, you can start to query against the Surreal deal dataset.", channel.id.as_u64()),
                                        None,
                                    ).await?;
                                    let db = db.clone();
                                    let (channel, ctx, command) =
                                        (channel.clone(), ctx.clone(), command.clone());
                                    tokio::spawn(async move {
                                        show(&ctx, &channel, crate::ConnType::EphemeralChannel, &config_clone).await.unwrap();
                                        db.import("premade/surreal_deal.surql").await.unwrap();
                                        channel.say(&ctx, format!("<@{}> Your instance now has Surreal deal dataset loaded, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                        channel
                                            .send_files(
                                                ctx,
                                                [AttachmentType::Path(Path::new(
                                                    "premade/surreal_deal.png",
                                                ))],
                                                |m| m.content("schema:"),
                                            )
                                            .await
                                            .unwrap();
                                    }.in_current_span());
                                }
                                dataset => {
                                    warn!(dataset, "Unknown dataset was requested");
                                    CmdError::UnknownDataset(dataset.to_string())
                                        .reply(&ctx, command)
                                        .await?;
                                    return Ok(());
                                }
                            }
                        }
                        CommandOptionType::Attachment => {
                            if let Some(CommandDataOptionValue::Attachment(attachment)) =
                                op_option.resolved
                            {
                                ephemeral_interaction(&ctx, command,
                                    "Database instance created, loading dataset...",
                                    format!("You now have your own database instance! Head over to <#{}> while your file is now being uploaded.\nOnce you receive a confirmation, you can start querying against the imported dataset.", channel.id.as_u64()),
                                    None,
                                ).await?;
                                match attachment.download().await {
                                    Ok(data) => {
                                        ephemeral_interaction_edit(&ctx, command, "Attachment downloaded, importing...", "Your attachment has been downloaded and is being imported.", None).await?;
                                        let db = db.clone();
                                        let (channel, ctx, command) =
                                            (channel.clone(), ctx.clone(), command.clone());
                                        tokio::spawn(async move {
                                            show(&ctx, &channel, crate::ConnType::EphemeralChannel, &config_clone).await.unwrap();
                                            if let Err(why) = db
                                                .query(String::from_utf8_lossy(&data).into_owned())
                                                .await
                                            {
                                                ephemeral_interaction_edit(&ctx, &command, "Error importing from file", format!("Error importing from file, please ensure that files are valid SurrealQL:\n```rust\n{}\n```", why), Some(false)).await.unwrap();
                                                channel.say(&ctx, format!(":x: Error loading data, channel will be deleted: {why}")).await.ok();
                                                channel.delete(ctx).await.ok();
                                                return;
                                            }
                                            channel.say(&ctx, format!("<@{}> Your instance now has your dataset, try writing some SurrealQL!", command.user.id.as_u64())).await.unwrap();
                                            ephemeral_interaction_edit(&ctx, &command, "Import completed", format!("Your attachment has been imported, head over to <#{}> to start writing SurrealQL to query your data!.", channel.id.as_u64()), Some(true)).await.unwrap();
                                        }.in_current_span());
                                    }
                                    Err(why) => {
                                        return CmdError::AttachmentDownload(why.into())
                                            .edit(&ctx, command)
                                            .await
                                    }
                                }
                            } else {
                                return ephemeral_interaction_edit(
                                    &ctx,
                                    command,
                                    "Error with attachment",
                                    "Unknown error with attachment",
                                    Some(false),
                                )
                                .await;
                            }
                        }
                        opt => {
                            CmdError::UnexpectedArgumentType(opt)
                                .reply(&ctx, command)
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Ordering::Less => {
                    show(
                        &ctx,
                        &channel,
                        crate::ConnType::EphemeralChannel,
                        &config_clone,
                    )
                    .await?;
                    ephemeral_interaction(
                        &ctx,
                        command,
                        "Database instance created",
                        format!(
                            "You now have your own database instance! Head over to <#{}> to start writing SurrealQL!",
                            channel.id.as_u64()
                        ),
                        None,
                    ).await?;
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
            Ok(())
        }
        None => CmdError::NoGuild.reply(&ctx, command).await,
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
