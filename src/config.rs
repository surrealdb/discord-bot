use serde::{Deserialize, Serialize};
use serenity::model::prelude::{ChannelId, GuildId};

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub guild_id: GuildId,
    pub active_channel: ChannelId,
    pub archive_channel: ChannelId,
}
