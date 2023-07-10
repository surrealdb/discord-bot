use anyhow::Error;
use serenity::model::prelude::*;

use crate::{channel_info::ChannelInfo, config::Config, DB};

pub async fn get_config(guild_id: GuildId) -> Result<Option<Config>, surrealdb::Error> {
    DB.select(("guild_config", guild_id.to_string())).await
}

pub async fn get_channel_info(
    channel_id: ChannelId,
) -> Result<Option<ChannelInfo>, surrealdb::Error> {
    DB.select(("channel_info", channel_id.to_string())).await
}
