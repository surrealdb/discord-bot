use serde::{Deserialize, Serialize};
use serenity::model::prelude::{ChannelId, GuildId};
use tokio::time::Duration;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub guild_id: GuildId,
    pub active_channel: ChannelId,
    pub archive_channel: ChannelId,
    // #[serde(default = "Duration::from_secs(20 * 60)")]
    pub ttl: Duration,
    pub pretty: bool,
    pub json: bool,
}
