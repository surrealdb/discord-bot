use crate::{
    config::Config,
    utils::{failure_ephemeral_interaction, success_ephemeral_interaction}, DB,
};

use anyhow::Result;
use humantime::format_duration;
use serenity::{
    model::prelude::{
        component::{
            ActionRow, ActionRowComponent,
            ButtonStyle::Success,
            InputText,
            InputTextStyle::Short,
        },
        message_component::MessageComponentInteraction,
        modal::ModalSubmitInteraction,
        ChannelId, GuildId,
        InteractionResponseType::Modal,
    },
    prelude::{Context, Mentionable},
};

pub async fn show(ctx: &Context, channel: &ChannelId, config: &Config) -> Result<()> {
    let version = DB.version().await?;

    channel.send_message(&ctx, |message| {
        message
        .embed(|embed| {
            embed
            .title("Your SurrealDB session")
            .description("This is your SurrealDB server configuration.\nYou can change it by clicking the options below.\n\nRight now active/archived channel groups can only be changed via `/config_update`.")
            .footer(|f| {
                f.text(format!("SurrealDB Version: {}", version)).icon_url("https://cdn.discordapp.com/icons/902568124350599239/cba8276fd365c07499fdc349f55535be.webp?size=240")
            })
            .field("Active Channel group is", config.active_channel.mention(), true)
            .field("Archived Channel group is", config.archive_channel.mention(), true)
            .field("Session lifetime after last query is ", format_duration(config.ttl), true)
            .field("Query timeout is set to ", format_duration(config.timeout), true)
            .field("Output format is ", if config.json { "JSON" } else { "SQL-like" }, true)
            .field("Output is ", if config.pretty { "prettified" } else { "raw" }, true)
        })
        .components(|c| {
            c.create_action_row(|r| {
                r.create_select_menu(|s| {
                    s.custom_id("configurable_server:format").placeholder("Select output format").min_values(1).max_values(1).options(|o| {
                        o.create_option(|o| o.default_selection(config.json).label("JSON format").value("json"))
                         .create_option(|o| o.default_selection(!config.json).label("SQL-like format").value("sql"))
                    })
                })
            }).create_action_row(|r|{
                r.create_select_menu(|s| {
                    s.custom_id("configurable_server:prettify").placeholder("Prettify output").min_values(1).max_values(1).options(|o| {
                        o.create_option(|o| o.default_selection(config.pretty).label("Pretty output").value("true"))
                         .create_option(|o| o.default_selection(!config.pretty).label("Raw output").value("false"))
                    })
                })
            }).create_action_row(|r| {
                r
                .create_button(|b| b.custom_id("configurable_server:ttl").label("Change TTL").style(Success).emoji('⏳'))
                .create_button(|b| b.custom_id("configurable_server:timeout").label("Change Query timeout").style(Success).emoji('⌛'))
            })
        })
    }).await?;

    Ok(())
}

#[instrument(skip(ctx, event), fields(user = %event.user))]
pub async fn handle_component(
    ctx: &Context,
    event: &MessageComponentInteraction,
    guild: &GuildId,
    id: &str,
    values: &[String],
) -> Result<()> {
    if !event
        .member
        .as_ref()
        .ok_or(anyhow::anyhow!("Not in a guild"))?
        .permissions
        .expect("we are in an interaction")
        .manage_channels()
    {
        info!("User tried to change config, but has no permissions");
        failure_ephemeral_interaction(
            &ctx,
            &event.id,
            &event.token,
            "No permissions!",
            "You need to have `Manage Channels` permission to change the server config!",
        )
        .await?;
        return Ok(());
    }

    let result: Result<Option<Config>, surrealdb::Error> =
        DB.select(("guild_config", guild.to_string())).await;

    let mut dirty = false;

    match (id, result) {
        (_, Ok(None)) => {
            info!("Tried to change config, but there is no config for this server.");
            failure_ephemeral_interaction(
                &ctx,
                &event.id,
                &event.token,
                "No config!",
                "There is no config for this server!\nPlease run `/configure` with initial settings!",
            )
            .await?;
            return Ok(());
        }
        (_, Err(err)) => {
            error!("Error while getting config: {}", err);
            failure_ephemeral_interaction(
                &ctx,
                &event.id,
                &event.token,
                "Error!",
                format!("Error while getting config:\n ```rust\n{err}\n```"),
            )
            .await?;
            return Ok(());
        }
        ("ttl", Ok(Some(config))) | ("timeout", Ok(Some(config))) => {
            event
                .create_interaction_response(&ctx, |a| {
                    a.kind(Modal).interaction_response_data(|d| {
                        d.components(|c| {
                            c.create_action_row(|r| {
                                r.create_input_text(|i| {
                                    i.custom_id(format!("configurable_server:{id}"))
                                        .label("Duration (5m 30s)")
                                        .style(Short)
                                        .value(match id {
                                            "ttl" => format_duration(config.ttl),
                                            "timeout" => format_duration(config.timeout),
                                            _ => unreachable!(),
                                        })
                                        .placeholder("Duration (5m30s)")
                                        .min_length(1)
                                        .max_length(100)
                                        .required(true)
                                })
                            })
                        })
                        .custom_id(format!("configurable_server:{id}"))
                        .title(format!("Setting new duration for {id}"))
                    })
                })
                .await?;
        }
        ("format", Ok(Some(mut config))) | ("prettify", Ok(Some(mut config))) => {
            match id {
                "format" => config.json = values[0] == "json",
                "prettify" => config.pretty = values[0] == "true",
                _ => unreachable!(),
            }
            let updated: Result<Option<Config>, surrealdb::Error> = DB
                .update(("guild_config", guild.to_string()))
                .content(config)
                .await;
            match updated {
                Ok(Some(_)) => {
                    dirty = true;
                    success_ephemeral_interaction(
                        &ctx,
                        &event.id,
                        &event.token,
                        "Config updated!",
                        format!("{} is now set to {}", id, values[0]),
                    )
                    .await?;
                }
                Ok(None) => {
                    unreachable!(
                        "Update returned None even though it should have always returned Some"
                    )
                }
                Err(err) => {
                    error!("Error while updating config: {}", err);
                    failure_ephemeral_interaction(
                        &ctx,
                        &event.id,
                        &event.token,
                        "Error!",
                        format!("Error while updating config:\n ```rust\n{err}\n```"),
                    )
                    .await?;
                }
            }
        }
        (_, _) => {
            unreachable!()
        }
    }

    // If we changed something, delete the original message and show a new one
    if dirty {
        let result: Result<Option<Config>, surrealdb::Error> =
            DB.select(("guild_config", guild.to_string())).await;
        match result {
            Ok(Some(config)) => {
                event.message.delete(&ctx).await?;
                show(&ctx, &event.channel_id, &config).await?;
            }
            _ => unreachable!("Should've returned long before..."),
        }
    }
    Ok(())
}

#[instrument(skip(ctx, event))]
pub async fn handle_modal(
    ctx: &Context,
    event: &ModalSubmitInteraction,
    guild: &GuildId,
    id: &str,
    values: &[ActionRow],
) -> Result<()> {
    match id {
        // Use humantime.parse_duration to parse component values.
        "ttl" | "timeout" => {
            if let ActionRowComponent::InputText(InputText { value, .. }) = &values[0].components[0]
            {
                let duration = match humantime::parse_duration(value) {
                    Ok(duration) => duration,
                    Err(err) => {
                        error!("Error while parsing duration: {}", err);
                        failure_ephemeral_interaction(
                            &ctx,
                            &event.id,
                            &event.token,
                            "Error while parsing duration",
                            format!("Error while parsing duration:\n ```rust\n{err}\n```"),
                        )
                        .await?;
                        return Ok(());
                    }
                };
                let result: Result<Option<Config>, surrealdb::Error> =
                    DB.select(("guild_config", guild.to_string())).await;
                match result {
                    Ok(Some(mut config)) => {
                        match id {
                            "ttl" => config.ttl = duration,
                            "timeout" => config.timeout = duration,
                            _ => unreachable!(),
                        }
                        let updated: Result<Option<Config>, surrealdb::Error> = DB
                            .update(("guild_config", guild.to_string()))
                            .content(config)
                            .await;
                        match updated {
                            Ok(Some(_)) => {
                                success_ephemeral_interaction(
                                    &ctx,
                                    &event.id,
                                    &event.token,
                                    "Config updated",
                                    format!(
                                        "{} is now set to {}",
                                        id,
                                        humantime::format_duration(duration),
                                    ),
                                )
                                .await?;
                            }
                            Ok(None) => {
                                unreachable!(
                                    "Update returned None even though it should have always returned Some"
                                )
                            }
                            Err(err) => {
                                error!("Error while updating config: {}", err);
                                failure_ephemeral_interaction(
                                    &ctx,
                                    &event.id,
                                    &event.token,
                                    "Error!",
                                    format!("Error while updating config:\n ```rust\n{err}\n```"),
                                )
                                .await?;
                            }
                        }
                    }
                    _ => unreachable!("Should've returned long before..."),
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
