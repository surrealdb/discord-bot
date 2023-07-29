use serenity::model::prelude::*;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;
use std::path::Path;
use tracing::error;

use surrealdb::engine::local::{Mem, RocksDb};

use surreal_bot::handler::Handler;
use surreal_bot::DB;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_default(),
        ))
        .init();

    match env::var("CONFIG_DB_PATH") {
        Ok(path) => {
            let path = Path::new(&path);
            DB.connect::<RocksDb>(path).await?;
        }
        Err(_) => {
            DB.connect::<Mem>(()).await?;
        }
    }
    DB.use_ns("SurrealBot").use_db("SurrealBot").await?;

    let token = env::var("DISCORD_TOKEN")?;

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        error!(error = %why, "An error occurred while running the client");
    }
    Ok(())
}
