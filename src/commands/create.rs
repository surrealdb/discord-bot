use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::{Guild, GuildChannel};
use serenity::prelude::Context;
use serenity::{builder::CreateApplicationCommand, model::prelude::ChannelType};
use surrealdb::engine::local::Mem;
use surrealdb::Surreal;
use tokio::time::{sleep_until, Instant};

use crate::config::Config;
use crate::{DB, DBCONNS, DEFAULT_TTL};

pub async fn run(command: &ApplicationCommandInteraction, ctx: Context) -> String {
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return "No config found for this server, please ask an administrator to configure the bot".to_string()
                    }
                }
                Err(e) => return format!("Database error: {}", e),
            };

            let guild = Guild::get(&ctx, id).await.unwrap();
            let channel = guild
                .create_channel(&ctx, |c| {
                    c.name(command.id.to_string())
                        .kind(ChannelType::Text)
                        .category(config.active_channel)
                })
                .await
                .unwrap();
            let db = Surreal::new::<Mem>(()).await.unwrap();
            db.use_ns("test").use_db("test").await.unwrap();
            DBCONNS.lock().await.insert(
                channel.id.as_u64().clone(),
                crate::Conn {
                    db: db,
                    last_used: Instant::now(),
                    ttl: DEFAULT_TTL.clone(),
                },
            );

            channel.say(&ctx, format!("This channel is now connected to a SurrealDB instance, try writing some SurrealQL!!!\n(note this will expire in {:#?})", DEFAULT_TTL)).await.unwrap();
            let res = format!("You now have you're own database instance, head over to <#{}> to start writing SurrealQL!!!", channel.id.as_u64());
            tokio::spawn(async move {
                let mut last_time: Instant = Instant::now();
                let mut ttl = DEFAULT_TTL.clone();
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
            return res;
        }
        None => {
            return "Direct messages are not currently supported".to_string();
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("create")
        .description("Creates a channel with a SurrealDB instance")
}

async fn clean_channel(channel: GuildChannel, ctx: &Context) {
    let _ = channel
        .say(
            &ctx,
            "This database instance has expired and is no longer functional",
        )
        .await;

    DBCONNS.lock().await.remove(channel.id.as_u64());
}
