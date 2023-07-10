use serde::{Deserialize, Serialize};
use serenity::model::prelude::{ChannelId, GuildId, UserId};

#[derive(Deserialize, Serialize, Debug)]
pub struct ChannelInfo {
    pub guild_id: GuildId,
    pub creator: UserId,
    pub state: ChannelState,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ChannelState {
    Active,
    Archived,
}
