use std::{cmp::Ordering, env, path::Path};

use cargo_lock::package::{GitReference, SourceKind};
use once_cell::sync::Lazy;
use serenity::{
    builder::CreateInteractionResponse,
    json::{self, Value},
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
            },
            AttachmentType, ChannelId, GuildChannel, InteractionId, InteractionResponseType,
            Message, PermissionOverwrite, PermissionOverwriteType,
        },
        user::User,
        Permissions,
    },
    prelude::Context,
};
use surrealdb::{
    engine::local::{Db, Mem},
    Surreal,
};
use tokio::{
    fs,
    time::{sleep_until, Instant},
};
use tracing::Instrument;

use crate::{config::Config, db_utils::get_config, Conn, ConnType, DBCONNS};

pub const MAX_FILE_SIZE: usize = 24_000_000;

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

/// Interaction-source independent function to create a success interaction message.
pub async fn success_ephemeral_interaction<DT: ToString, DD: ToString>(
    ctx: &Context,
    iid: &InteractionId,
    token: &str,
    title: DT,
    description: DD,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = CreateInteractionResponse::default();
    interaction_response.interaction_response_data(|d| {
        d.embed(|e| {
            e.title(title.to_string())
                .description(description.to_string())
                .color(0x00ff00)
        })
        .ephemeral(true)
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    ctx.http
        .create_interaction_response(iid.0, token, &Value::from(map))
        .await?;
    Ok(())
}

/// Interaction-source independent function to create a failure interaction message.
pub async fn failure_ephemeral_interaction<DT: ToString, DD: ToString>(
    ctx: &Context,
    iid: &InteractionId,
    token: &str,
    title: DT,
    description: DD,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = CreateInteractionResponse::default();
    interaction_response.interaction_response_data(|d| {
        d.embed(|e| {
            e.title(title.to_string())
                .description(description.to_string())
                .color(0xff0000)
        })
        .ephemeral(true)
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    ctx.http
        .create_interaction_response(iid.0, token, &Value::from(map))
        .await?;
    Ok(())
}

/// Interaction-source independent function to create a logged success interaction message for a specific user.
pub async fn success_user_interaction<DT: ToString, DD: ToString>(
    ctx: &Context,
    iid: &InteractionId,
    token: &str,
    user: &User,
    title: DT,
    description: DD,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = CreateInteractionResponse::default();
    interaction_response.interaction_response_data(|d| {
        d.embed(|e| {
            e.title(title.to_string())
                .description(description.to_string())
                .color(0x00ff00)
                .author(|a| {
                    a.name(&user.name)
                        .icon_url(user.avatar_url().unwrap_or_default())
                })
        })
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    ctx.http
        .create_interaction_response(iid.0, token, &Value::from(map))
        .await?;
    Ok(())
}

pub fn read_view_perms(kind: PermissionOverwriteType) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            .union(Permissions::SEND_MESSAGES)
            .union(Permissions::READ_MESSAGE_HISTORY),
        deny: Permissions::empty(),
        kind,
    }
}

#[instrument(skip_all, fields(guild_id = channel.guild_id.as_u64(), channel_id = channel.id.as_u64(), channel_name = channel.name.clone()))]
pub async fn clean_channel(mut channel: GuildChannel, ctx: &Context) {
    info!("Cleaning up channel");
    let entry = DBCONNS.lock().await.remove(channel.id.as_u64());

    if let Some(conn) = entry {
        channel
            .say(
                &ctx,
                ":information_source: This database instance has expired and is no longer functional",
            )
            .await
            .ok();

        let base_path = match env::var("TEMP_DIR_PATH") {
            Ok(p) => p,
            Err(_) => {
                fs::create_dir("tmp").await.ok();
                "tmp/".to_string()
            }
        };
        let path = format!("{base_path}{}.surql", channel.id.as_u64());

        match conn.db.export(&path).await {
            Ok(_) => {
                if let Ok(metadata) = fs::metadata(&path).await {
                    if metadata.len() < MAX_FILE_SIZE as u64 {
                        channel
                            .send_message(&ctx, |m| {
                                m.content("Database exported:").add_file(Path::new(&path))
                            })
                            .await
                            .ok();
                    } else {
                        channel.send_message(&ctx, |m| m.content(":x: Your database is too powerful, (the export is too large to send)")).await.ok();
                    }
                }

                fs::remove_file(path).await.ok();
            }
            Err(why) => {
                channel
                    .send_message(&ctx, |m| {
                        m.content(format!(":x: Database export failed: {why}"))
                    })
                    .await
                    .ok();
            }
        };
    }

    let result = get_config(channel.guild_id).await;

    let response = match result {
        Ok(o) => o,
        Err(_) => return,
    };

    let config = match response {
        Some(c) => c,
        None => return,
    };

    if Some(config.active_channel) == channel.parent_id {
        channel
            .edit(ctx, |c| c.category(config.archive_channel))
            .await
            .ok();
    }

    channel
        .edit_thread(ctx, |thread| {
            thread.archived(true).auto_archive_duration(60)
        })
        .await
        .ok();
}

#[instrument(skip_all)]
pub async fn register_db(
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
    config: Config,
    conn_type: ConnType,
    require_query: bool,
) -> Result<(), anyhow::Error> {
    info!("Registering a new database");
    DBCONNS.lock().await.insert(
        *channel.id.as_u64(),
        crate::Conn {
            db,
            last_used: Instant::now(),
            conn_type,
            ttl: config.ttl,
            pretty: config.pretty,
            json: config.json,
            require_query,
        },
    );

    tokio::spawn(
        async move {
            let mut last_time;
            let mut ttl;
            loop {
                match DBCONNS.lock().await.get(channel.id.as_u64()) {
                    Some(e) => {
                        last_time = e.last_used;
                        ttl = e.ttl
                    }
                    None => {
                        break;
                    }
                }
                if last_time.elapsed() >= ttl {
                    clean_channel(channel, &ctx).await;
                    break;
                }
                sleep_until(last_time + ttl).await;
            }
        }
        .instrument(tracing::Span::current()),
    );
    Ok(())
}

pub async fn respond(
    reply: String,
    ctx: Context,
    query_msg: Message,
    conn: &Conn,
    channel_id: ChannelId,
) -> Result<(), anyhow::Error> {
    if reply.len() < 1900 {
        query_msg
            .reply(
                &ctx,
                format!(
                    "```{}\n{}\n```",
                    if conn.json { "json" } else { "sql" },
                    reply
                ),
            )
            .await
            .unwrap();
    } else {
        let mut truncated = false;
        let data = match reply.as_bytes().len().cmp(&MAX_FILE_SIZE) {
            Ordering::Equal | Ordering::Less => reply.as_bytes(),
            Ordering::Greater => {
                truncated = true;
                reply.as_bytes().split_at(MAX_FILE_SIZE).0
            }
        };
        let reply_attachment = AttachmentType::Bytes {
            data: std::borrow::Cow::Borrowed(data),
            filename: format!("response.{}", if conn.json { "json" } else { "sql" }),
        };
        channel_id
            .send_message(&ctx, |m| {
                let message = m.reference_message(&query_msg).add_file(reply_attachment);
                if truncated {
                    message.content(
                        ":information_source: Response was too long and has been truncated",
                    )
                } else {
                    message
                }
            })
            .await
            .unwrap();
    }
    Ok(())
}

pub async fn load_attachment(
    op_option: CommandDataOption,
    command: &ApplicationCommandInteraction,
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
) -> Result<(), anyhow::Error> {
    if let Some(CommandDataOptionValue::Attachment(attachment)) = op_option.resolved {
        interaction_reply(
            command,
            ctx.clone(),
            format!(
                ":information_source: Your file ({}) is now being downloaded!",
                attachment.filename
            ),
        )
        .await?;
        match attachment.download().await {
            Ok(data) => {
                interaction_reply_edit(command, ctx.clone(), ":information_source: Your data is currently being loaded, soon you'll be able to query your dataset! \n_Please wait for a confirmation that the dataset is loaded!_".to_string()).await?;

                let db = db.clone();
                let (_channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
                tokio::spawn(async move {
                    if let Err(why) = db.query(String::from_utf8_lossy(&data).into_owned()).await {
                        interaction_reply_edit(
                            &command,
                            ctx,
                            format!(":x: Error importing from file, please ensure that files are valid SurrealQL: {}", why),
                        )
                        .await
                        .unwrap();
                        return;
                    }
                    interaction_reply_edit(
                        &command,
                        ctx,
                        ":information_source: Your data is now loaded and ready to query!".to_string(),
                    )
                    .await
                    .unwrap();
                }.instrument(tracing::Span::current()));
                Ok(())
            }
            Err(why) => {
                interaction_reply_edit(command, ctx, format!(":x: Error with attachment: {}", why))
                    .await?;
                Ok(())
            }
        }
    } else {
        interaction_reply_edit(command, ctx, ":x: Error with attachment").await?;
        Ok(())
    }
}

#[instrument]
pub async fn create_db_instance(server_config: &Config) -> Result<Surreal<Db>, anyhow::Error> {
    info!("Creating database instance");
    let db_config = surrealdb::opt::Config::new()
        .query_timeout(server_config.timeout)
        .transaction_timeout(server_config.timeout);
    let db = Surreal::new::<Mem>(db_config).await?;

    db.use_ns("test").use_db("test").await?;

    Ok(db)
}

pub const SURREALDB_VERSION: Lazy<String> = Lazy::new(|| {
    let lock: cargo_lock::Lockfile = include_str!("../Cargo.lock")
        .parse()
        .expect("Failed to parse Cargo.lock");
    let package = lock
        .packages
        .iter()
        .find(|p| p.name.as_str() == "surrealdb")
        .expect("Failed to find surrealdb in Cargo.lock");

    match &package.source {
        Some(source) => {
            let kind = match source.kind() {
                SourceKind::Git(git) => {
                    format!(
                        "git: {}",
                        match git {
                            GitReference::Branch(branch) => format!("branch: {}", branch),
                            GitReference::Tag(tag) => format!("tag: {}", tag),
                            GitReference::Rev(rev) => format!("rev: {}", rev),
                        }
                    )
                }
                SourceKind::Registry | SourceKind::LocalRegistry | SourceKind::SparseRegistry => {
                    "registry".to_string()
                }
                SourceKind::Path => "localpath".to_string(),
                _ => "unknown".to_string(),
            };
            format!("v{} ({kind})", package.version)
        }
        None => "unknown".to_string(),
    }
});
