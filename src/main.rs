use serenity::model::prelude::*;
use serenity::prelude::*;

use dotenvy::dotenv;
use std::env;
use std::path::Path;
use surreal_bot::config::Config;
use tracing::{error, info};

use surrealdb::engine::local::{Mem, RocksDb};

use surreal_bot::handler::Handler;
use surreal_bot::{stats, DB};

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
        Ok(path) => match path.to_lowercase().as_str() {
            "default" => {
                DB.connect::<Mem>(()).await?;
                let config = Config::surrealdb_default();
                let _: Option<Config> = DB
                    .create(("guild_config", config.guild_id.to_string()))
                    .content(config)
                    .await
                    .ok()
                    .flatten();
            }
            _ => {
                let path = Path::new(&path);
                DB.connect::<RocksDb>(path).await?;
            }
        },
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

    let shard_manager = client.shard_manager.clone();
    let http = client.cache_and_http.http.clone();
    stats::start(http.clone());
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");

        match surreal_bot::shutdown(&http).await {
            Ok(_) => info!("Surreal Bot DBCONNS exported successfully"),
            Err(e) => {
                error!(error = %e, "An error occurred while shutting down");
            }
        }
        shard_manager.lock().await.shutdown_all().await;
    });

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        error!(error = %why, "An error occurred while running the client");
    }
    Ok(())
}
