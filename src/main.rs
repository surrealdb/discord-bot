use std::collections::HashMap;
use std::fs;

use serenity::async_trait;
use serenity::builder::CreateChannel;
use serenity::model::channel::Message;
use serenity::model::prelude::*;
use serenity::prelude::*;

use toml;

use once_cell::sync::Lazy;
use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

fn validate_msg(msg: &Message) -> bool {
    if msg.author.bot == true {
        return false;
    };
    true
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // println!("{:?}", msg);
        // println!("{:?}", msg.content);

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
                    DBCONNS.lock().await.insert(channel.id.as_u64().clone(), db);

                    channel.say(&ctx, "This channel is now connected to a SurrealDB instance, try writing some SurrealQL!!!").await.unwrap();
                }
                None => {
                    msg.reply(&ctx, "Direct messages are not currently supported")
                        .await
                        .unwrap();
                    return;
                }
            }
        } else if let Some(db) = DBCONNS.lock().await.get(msg.channel_id.as_u64()) {
            let result = db.query(&msg.content).await;
            if validate_msg(&msg) {
                msg.reply(&ctx, format!("{:#?}", result)).await.unwrap();
            }
        } else {
            return;
        }
    }
}

// static DBCONNS: Mutex<HashMap<u64, Surreal<Db>>> = Mutex::new(HashMap::new());
static DBCONNS: Lazy<Mutex<HashMap<u64, Surreal<Db>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

static DB: Surreal<Db> = Surreal::init();

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    DB.connect::<Mem>(()).await?;
    DB.use_ns("test").use_db("test").await?;

    let secrets = fs::read_to_string("secrets.toml")
        .expect("expected secrets.toml file")
        .parse::<toml::Value>()
        .expect("expected valid json");

    let token = secrets["DISCORD_TOKEN"].as_str().expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
    Ok(())
}
