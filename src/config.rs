use serde::{Deserialize, Serialize};
use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommandOption},
    model::prelude::{
        application_command::{ApplicationCommandInteraction, CommandDataOption},
        command::CommandOptionType,
        ChannelId, ChannelType, GuildId,
    },
};
use tokio::time::Duration;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub guild_id: GuildId,
    pub active_channel: ChannelId,
    pub archive_channel: ChannelId,
    pub ttl: Duration,
    pub pretty: bool,
    pub json: bool,
}

impl Config {
    pub fn merge(&mut self, to_add: ConfigBuilder) {
        assert_eq!(self.guild_id, to_add.guild_id.unwrap());
        if let Some(active_channel) = to_add.active_channel {
            self.active_channel = active_channel;
        }
        if let Some(archive_channel) = to_add.archive_channel {
            self.archive_channel = archive_channel;
        }
        if let Some(ttl) = to_add.ttl {
            self.ttl = ttl;
        }
        if let Some(pretty) = to_add.pretty {
            self.pretty = pretty;
        }
        if let Some(json) = to_add.json {
            self.json = json;
        }
    }

    pub fn from_builder(builder: ConfigBuilder) -> Option<Config> {
        Some(Config {
            guild_id: builder.guild_id?,
            active_channel: builder.active_channel?,
            archive_channel: builder.archive_channel?,
            ttl: builder.ttl?,
            pretty: builder.pretty?,
            json: builder.json?,
        })
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ConfigBuilder {
    pub guild_id: Option<GuildId>,
    pub active_channel: Option<ChannelId>,
    pub archive_channel: Option<ChannelId>,
    pub ttl: Option<Duration>,
    pub pretty: Option<bool>,
    pub json: Option<bool>,
}

impl ConfigBuilder {
    pub fn build(command: &ApplicationCommandInteraction) -> Self {
        let options = command.data.options.clone();
        let mut acc = Self::empty();
        acc.guild_id = command.guild_id;

        for option in options {
            match option.name.as_str() {
                "active" => {
                    acc.active_channel = Some(ChannelId(
                        option.value.unwrap().as_str().unwrap().parse().unwrap(),
                    ))
                }
                "archive" => {
                    acc.archive_channel = Some(ChannelId(
                        option.value.unwrap().as_str().unwrap().parse().unwrap(),
                    ))
                }
                "ttl" => {
                    acc.ttl = Some(Duration::from_secs(
                        option.value.clone().unwrap().as_u64().unwrap(),
                    ))
                }
                "pretty" => acc.pretty = Some(option.value.clone().unwrap().as_bool().unwrap()),
                "json" => acc.json = Some(option.value.clone().unwrap().as_bool().unwrap()),
                _ => {}
            }
        }

        acc
    }

    fn empty() -> Self {
        Self {
            guild_id: None,
            active_channel: None,
            archive_channel: None,
            ttl: None,
            pretty: None,
            json: None,
        }
    }
}

pub fn register_options(
    command: &mut CreateApplicationCommand,
    req: bool,
) -> &mut CreateApplicationCommand {
    command
        .create_option(|option| {
            option
                .name("active")
                .description("channel category for current database instances")
                .kind(CommandOptionType::Channel)
                .channel_types(&[ChannelType::Category])
                .required(req)
        })
        .create_option(|option| {
            option
                .name("archive")
                .description("channel category for archived database instances")
                .kind(CommandOptionType::Channel)
                .channel_types(&[ChannelType::Category])
                .required(req)
        })
        .create_option(|option| {
            option
                .name("ttl")
                .description("The default time to live for created channels in seconds")
                .kind(CommandOptionType::Integer)
                .required(req)
        })
        .create_option(|option| {
            option
                .name("pretty")
                .description("whether or not to pretty print responses")
                .kind(CommandOptionType::Boolean)
                .required(req)
        })
        .create_option(|option| {
            option
                .name("json")
                .description("whether to format output as JSON, the alternative is SurrealQL")
                .kind(CommandOptionType::Boolean)
                .default_option(false)
                .required(req)
        })
}
