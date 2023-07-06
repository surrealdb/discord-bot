use std::fs;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use toml;

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
        println!("{:?}", msg.content);
        let result = DB.query(&msg.content).await;
        if validate_msg(&msg) {
            msg.reply(ctx, format!("{:#?}", result)).await.unwrap();
        }
    }
}

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
