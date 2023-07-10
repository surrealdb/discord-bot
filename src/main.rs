use std::collections::HashMap;
use std::fs;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::prelude::*;
use serenity::prelude::*;

use toml;

use surrealdb::engine::local::Mem;

use surreal_bot::hander::Handler;
use surreal_bot::DB;

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
