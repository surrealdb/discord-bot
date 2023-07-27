use std::{cmp::Ordering, path::Path};

use serenity::{
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
            },
            AttachmentType, ChannelId, GuildChannel, InteractionResponseType, Message,
            PermissionOverwrite, PermissionOverwriteType,
        },
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

pub fn read_view_perms(kind: PermissionOverwriteType) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            .union(Permissions::SEND_MESSAGES)
            .union(Permissions::READ_MESSAGE_HISTORY),
        deny: Permissions::empty(),
        kind: kind,
    }
}

pub async fn clean_channel(mut channel: GuildChannel, ctx: &Context) {
    let entry = DBCONNS.lock().await.remove(channel.id.as_u64());

    if let Some(conn) = entry {
        channel
            .say(
                &ctx,
                "This database instance has expired and is no longer functional",
            )
            .await
            .ok();

        fs::create_dir("tmp").await.ok();
        let path = format!("tmp/{}.surql", channel.id.as_u64());

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
                        channel.send_message(&ctx, |m| m.content("Your database is too powerful, (the export is too large to send)")).await.ok();
                    }
                }

                fs::remove_file(path).await.ok();
            }
            Err(why) => {
                channel
                    .send_message(&ctx, |m| {
                        m.content(format!("Database export failed: {why}"))
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

pub async fn register_db(
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
    config: Config,
    conn_type: ConnType,
    require_query: bool,
) -> Result<(), anyhow::Error> {
    DBCONNS.lock().await.insert(
        channel.id.as_u64().clone(),
        crate::Conn {
            db: db,
            last_used: Instant::now(),
            conn_type,
            ttl: config.ttl.clone(),
            pretty: config.pretty.clone(),
            json: config.json.clone(),
            require_query,
        },
    );

    tokio::spawn(async move {
        let mut last_time;
        let mut ttl;
        loop {
            match DBCONNS.lock().await.get(channel.id.as_u64()) {
                Some(e) => {
                    last_time = e.last_used;
                    ttl = e.ttl
                }
                None => {
                    clean_channel(channel, &ctx).await;
                    break;
                }
            }
            if last_time.elapsed() >= ttl {
                clean_channel(channel, &ctx).await;
                break;
            }
            sleep_until(last_time + ttl).await;
        }
    });
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
                    message.content("Response was too long and has been truncated")
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
                "Your file ({}) is now being downloaded!!!",
                attachment.filename
            ),
        )
        .await?;
        match attachment.download().await {
            Ok(data) => {
                interaction_reply_edit(command, ctx.clone(), format!("Your data is currently being loaded, soon you'll be able to query your dataset!!!")).await?;

                let db = db.clone();
                let (_channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
                tokio::spawn(async move {
                    if let Err(why) = db.query(String::from_utf8_lossy(&data).into_owned()).await {
                        interaction_reply_edit(
                            &command,
                            ctx,
                            format!("Error importing from file, please ensure that files are valid SurrealQL: {}", why),
                        )
                        .await
                        .unwrap();
                        return;
                    }
                    interaction_reply_edit(
                        &command,
                        ctx,
                        format!("Your data is now loaded and ready to query!!!"),
                    )
                    .await
                    .unwrap();
                });
                Ok(())
            }
            Err(why) => {
                interaction_reply_edit(command, ctx, format!("Error with attachment: {}", why))
                    .await?;
                Ok(())
            }
        }
    } else {
        interaction_reply_edit(command, ctx, "Error with attachment").await?;
        Ok(())
    }
}

pub async fn create_db_instance(server_config: &Config) -> Result<Surreal<Db>, anyhow::Error> {
    let db_config = surrealdb::opt::Config::new()
        .query_timeout(server_config.timeout)
        .transaction_timeout(server_config.timeout);
    let db = Surreal::new::<Mem>(db_config).await?;

    db.use_ns("test").use_db("test").await?;

    Ok(db)
}
