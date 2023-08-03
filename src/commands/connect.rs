use std::cmp::Ordering;
use std::path::Path;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::{AttachmentType, GuildChannel};
use serenity::prelude::Context;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tracing::Instrument;

use crate::components::configurable_session::show;
use crate::{premade, utils::*, DBCONNS};

use crate::config::Config;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if DBCONNS
        .lock()
        .await
        .contains_key(command.channel_id.as_u64())
    {
        CmdError::ExpectedNoSession.reply(&ctx, command).await?;
        return Ok(());
    }
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

            let channel = command.channel_id.to_channel(&ctx).await?.guild().unwrap();

            let db = create_db_instance(&config).await?;

            register_db(
                ctx.clone(),
                db.clone(),
                channel.clone(),
                config.clone(),
                crate::ConnType::ConnectedChannel,
                true,
            )
            .await?;

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
                                    load_premade(
                                        ctx,
                                        db,
                                        channel,
                                        command,
                                        "surreal_deal_mini.surql",
                                        "Surreal deal (mini)",
                                        Some("surreal_deal.png"),
                                        &config,
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
                                        &config,
                                    )
                                    .await?;
                                }
                                dataset => {
                                    CmdError::UnknownDataset(dataset.to_string())
                                        .reply(&ctx, command)
                                        .await?;
                                    return Ok(());
                                }
                            }
                        }
                        CommandOptionType::Attachment => {
                            show(&ctx, &channel, crate::ConnType::ConnectedChannel, &config)
                                .await?;
                            load_attachment(op_option, command, ctx, db, channel).await?
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
                    show(&ctx, &channel, crate::ConnType::ConnectedChannel, &config).await?;
                    ephemeral_interaction(
                        &ctx,
                        command,
                        "Session created",
                        "Your session is now available!",
                        Some(true),
                    )
                    .await?;
                }
            };
            Ok(())
        }
        None => CmdError::NoGuild.reply(&ctx, command).await,
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("connect")
        .description("Creates a SurrealDB instance and associates it with the current channel")
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
    config: &Config,
) -> Result<(), anyhow::Error> {
    {
        show(&ctx, &channel, crate::ConnType::ConnectedChannel, config).await?;
        ephemeral_interaction(&ctx, command,
            "Premade dataset loading...",
            format!("The dataset is currently being loaded, soon you'll be able to query the {} dataset! \n_Please wait for a confirmation that the dataset is loaded!_", name), None).await?;

        let db = db.clone();
        let (channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
        tokio::spawn(async move {
            match db.import(format!("premade/{}", file_name)).await {
                Ok(_) => {
                    ephemeral_interaction_edit(&ctx, &command,
                        "Premade dataset loaded!",
                        format!("The dataset is now loaded and you can query the {} dataset with the `/query` command!", name), Some(true)).await.unwrap();
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
                    ephemeral_interaction_edit(&ctx, &command, "Dataset loading failed!" , format!("Error loading data:\n```rust\n{}\n```", why), Some(false)).await.unwrap();
                }
            };
        }.in_current_span());
        Ok(())
    }
}
