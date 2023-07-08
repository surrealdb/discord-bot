use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::{sleep, sleep_until, Duration, Instant};

use surrealdb::engine::local::Mem;
use surrealdb::Surreal;

use crate::process;
use crate::DB;
use crate::{DBCONNS, DEFAULT_TTL};

fn validate_msg(msg: &Message) -> bool {
    if msg.author.bot == true {
        return false;
    };
    true
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

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!sql" {
            match msg.guild_id {
                Some(id) => {
                    let guild = Guild::get(&ctx, id).await.unwrap();
                    let channel = guild
                        .create_channel(&ctx, |c| {
                            c.name(msg.id.to_string()).kind(ChannelType::Text)
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
                    msg.reply(&ctx, format!("You now have you're own database instance, head over to <#{}> to start writing SurrealQL!!!", channel.id.as_u64())).await.unwrap();
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
                }
                None => {
                    msg.reply(&ctx, "Direct messages are not currently supported")
                        .await
                        .unwrap();
                    return;
                }
            }
        } else if let Some(conn) = DBCONNS.lock().await.get_mut(msg.channel_id.as_u64()) {
            conn.last_used = Instant::now();
            let result = conn.db.query(&msg.content).await;
            if validate_msg(&msg) {
                let reply = match process(true, true, result) {
                    Ok(r) => r,
                    Err(e) => e.to_string(),
                };
                msg.reply(&ctx, reply).await.unwrap();
            }
        } else {
            return;
        }
    }
}
