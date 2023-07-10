use std::fs;

use serenity::model::prelude::*;
use serenity::prelude::*;

use toml;

use surrealdb::engine::local::{File, Mem};

use surreal_bot::hander::Handler;
use surreal_bot::DB;

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    // DB.connect::<Mem>(()).await?;
    DB.connect::<File>("A:/_Coding/!SurrealDB/SurrealBot/database.db")
        .await?;
    DB.use_ns("SurrealBot").use_db("SurrealBot").await?;

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
