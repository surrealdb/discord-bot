use serenity::{
    model::{
        prelude::{
            application_command::ApplicationCommandInteraction, GuildChannel,
            InteractionResponseType, PermissionOverwrite, PermissionOverwriteType,
        },
        Permissions,
    },
    prelude::Context,
};
use surrealdb::{engine::local::Db, Surreal};
use tokio::time::{sleep_until, Instant};

use crate::{config::Config, db_utils::get_config, ConnType, DBCONNS};

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
    if DBCONNS.lock().await.contains_key(channel.id.as_u64()) {
        channel
            .say(
                &ctx,
                "This database instance has expired and is no longer functional",
            )
            .await
            .ok();

        DBCONNS.lock().await.remove(channel.id.as_u64());
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
