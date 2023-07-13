use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::Guild;

use serenity::prelude::Context;
use surrealdb::engine::local::Mem;
use surrealdb::Surreal;
use tokio::time::{sleep_until, Instant};

use crate::utils::*;
use crate::ConnType;

use crate::config::Config;
use crate::utils::interaction_reply;
use crate::{DB, DBCONNS};

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
                        None => return interaction_reply(command, ctx, "No config found for this server, please ask an administrator to configure the bot".to_string()).await
                    }
                }
                Err(e) => return interaction_reply(command, ctx, format!("Database error: {}", e)).await,
            };

            println!("options array length:{:?}", command.data.options.len());

            let message = command.data.resolved.messages.keys().next().unwrap();

            let channel = command
                .channel_id
                .create_public_thread(&ctx, message, |t| t.name(command.id.to_string()))
                .await?;

            let db = Surreal::new::<Mem>(()).await.unwrap();
            db.use_ns("test").use_db("test").await.unwrap();

            channel.say(&ctx, format!("This public thread is now connected to a SurrealDB instance, try writing some SurrealQL!!!\nuse /load to load a premade dataset or your own SurrealQL\n(note this will expire in {:#?})", config.ttl)).await?;
            interaction_reply(command, ctx.clone(), format!("You now have you're own database instance, head over to <#{}> to start writing SurrealQL!!!", channel.id.as_u64())).await?;

            DBCONNS.lock().await.insert(
                channel.id.as_u64().clone(),
                crate::Conn {
                    db: db,
                    last_used: Instant::now(),
                    conn_type: ConnType::Thread,
                    ttl: config.ttl.clone(),
                    pretty: config.pretty.clone(),
                    json: config.json.clone(),
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
            return Ok(());
        }
        None => {
            return interaction_reply(
                command,
                ctx,
                "Direct messages are not currently supported".to_string(),
            )
            .await;
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("create_db_thread")
        .kind(serenity::model::prelude::command::CommandType::Message)
}
