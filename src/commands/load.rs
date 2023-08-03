use std::cmp::Ordering;
use std::path::Path;

use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::{AttachmentType, GuildChannel};

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::Context;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tokio::time::Instant;
use tracing::Instrument;

use crate::premade;

use crate::utils::{ephemeral_interaction, ephemeral_interaction_edit, load_attachment, CmdError};
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if command.data.options.is_empty() {
        return CmdError::ExpectedArgument("a file or premade dataset to load".to_string())
            .reply(&ctx, command)
            .await;
    }
    match command.guild_id {
        Some(_guild_id) => {
            let channel = command.channel_id.to_channel(&ctx).await?.guild().unwrap();

            let db = match DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
                Some(c) => {
                    c.last_used = Instant::now();
                    c.db.clone()
                }
                None => return CmdError::NoSession.reply(&ctx, command).await,
            };

            match command.data.options.len().cmp(&1) {
                Ordering::Greater => {
                    return CmdError::TooManyArguments(1, command.data.options.len())
                        .reply(&ctx, command)
                        .await;
                }
                Ordering::Equal => {
                    let op_option = command.data.options[0].clone();
                    match op_option.kind {
                        CommandOptionType::String => {
                            match op_option.value.unwrap().as_str().unwrap() {
                                "surreal_deal_mini" => {
                                    load_premade(
                                        ctx,
                                        db,
                                        channel,
                                        command,
                                        "surreal_deal_mini.surql",
                                        "Surreal deal (mini)",
                                        Some("surreal_deal.png"),
                                    )
                                    .await?;
                                }
                                "surreal_deal" => {
                                    load_premade(
                                        ctx,
                                        db,
                                        channel,
                                        command,
                                        "surreal_deal.surql",
                                        "Surreal deal",
                                        Some("surreal_deal.png"),
                                    )
                                    .await?;
                                }
                                dataset => {
                                    warn!(dataset, "Unknown dataset was requested");
                                    return CmdError::UnknownDataset(dataset.to_string())
                                        .reply(&ctx, command)
                                        .await;
                                }
                            }
                        }
                        CommandOptionType::Attachment => {
                            load_attachment(op_option, command, ctx, db, channel).await?
                        }
                        opt => {
                            return CmdError::UnexpectedArgumentType(opt)
                                .reply(&ctx, command)
                                .await
                        }
                    }
                }
                Ordering::Less => panic!(),
            };

            Ok(())
        }
        None => CmdError::NoGuild.reply(&ctx, command).await,
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("load")
        .description("load data into a channel")
        .create_option(premade::register)
        .create_option(|option| {
            option
                .name("file")
                .description("A SurrealQL file to load into the database instance")
                .kind(CommandOptionType::Attachment)
                .required(false)
        })
}

async fn load_premade(
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
    command: &ApplicationCommandInteraction,
    file_name: &'static str,
    name: &'static str,
    schema_name: Option<&'static str>,
) -> Result<(), anyhow::Error> {
    {
        ephemeral_interaction(&ctx, command,
            "Loading premade dataset...",
            format!("The dataset is currently being loaded, soon you'll be able to query the {name} dataset! \n_Please wait for a confirmation that the dataset is loaded!_"),
            None,
        ).await?;

        let db = db.clone();
        let (channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
        tokio::spawn(
            async move {
                match db.import(format!("premade/{}", file_name)).await {
                    Ok(_) => {
                        ephemeral_interaction_edit(
                            &ctx,
                            &command,
                            "Premade dataset loaded!",
                            format!(
                                "The dataset is now loaded and you can query the {name} dataset!"
                            ),
                            Some(true),
                        )
                        .await
                        .unwrap();
                        if let Some(scheme_file_name) = schema_name {
                            channel
                                .send_files(
                                    ctx,
                                    [AttachmentType::Path(Path::new(&format!(
                                        "premade/{}",
                                        scheme_file_name
                                    )))],
                                    |m| m.content("schema:"),
                                )
                                .await
                                .unwrap();
                        }
                    }
                    Err(why) => {
                        ephemeral_interaction_edit(
                            &ctx,
                            &command,
                            "Error loading premade dataset!",
                            format!("Error loading data:\n```rust\n{why}\n```"),
                            Some(false),
                        )
                        .await
                        .unwrap();
                    }
                };
            }
            .in_current_span(),
        );
        Ok(())
    }
}
