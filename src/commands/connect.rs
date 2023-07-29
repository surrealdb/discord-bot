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

use crate::{premade, utils::*, DBCONNS};

use crate::config::Config;
use crate::utils::{interaction_reply, interaction_reply_edit, interaction_reply_ephemeral};
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
        interaction_reply_ephemeral(
            command,
            ctx,
            ":information_source: This channel already has an associated database instance",
        )
        .await?;
        return Ok(());
    }
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return interaction_reply_ephemeral(command, ctx, ":warning: No config found for this server, please ask an administrator to configure the bot!".to_string()).await
                    }
                }
                Err(e) => return interaction_reply_ephemeral(command, ctx, format!(":x: Database error: {}", e)).await,
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
                    interaction_reply_ephemeral(command, ctx, ":information_source: Please only supply one arguement (you can use the up arrow to edit the previous command)").await?;
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
                                _ => {
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
                        CommandOptionType::Attachment => load_attachment(op_option, command, ctx, db, channel).await?,
                        _ => {
                            interaction_reply_ephemeral(command, ctx, ":x: Unsupported option type")
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Ordering::Less => interaction_reply(command, ctx, format!(":information_source: This channel is now connected to a SurrealDB instance, try writing some SurrealQL with the `/query` command! \n_Please note this channel will expire after {:#?} of inactivity._", config.ttl)).await?,
            };
            Ok(())
        }
        None => {
            interaction_reply(
                command,
                ctx,
                ":warning: Direct messages are not currently supported".to_string(),
            )
            .await
        }
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
) -> Result<(), anyhow::Error> {
    {
        interaction_reply(
            command,
            ctx.clone(),
            format!(
                ":information_source: The dataset is currently being loaded, soon you'll be able to query the {} dataset! \n_Please wait for a confirmation that the dataset is loaded!_",
                name
            ),
        )
        .await?;
        let db = db.clone();
        let (channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
        tokio::spawn(async move {
            match db.import(format!("premade/{}", file_name)).await {
                Ok(_) => {
                    interaction_reply_edit(
                        &command,
                        ctx.clone(),
                        format!(
                            ":white_check_mark: The dataset is now loaded and you can query the {} dataset with the `/query` command!",
                            name
                        ),
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
                    interaction_reply_edit(&command, ctx, format!(":x: Error loading data: {}", why))
                        .await
                        .unwrap();
                }
            };
        }.instrument(tracing::Span::current()));
        Ok(())
    }
}
