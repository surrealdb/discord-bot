use std::{borrow::Cow, sync::Arc};

use crate::{
    config::Config,
    utils::{
        clean_channel, ephemeral_interaction, user_interaction, CmdError, BOT_VERSION,
        SURREALDB_VERSION,
    },
    ConnType, BIG_QUERY_SENT_KEY, BIG_QUERY_VARS_KEY, DBCONNS,
        clean_channel, failure_ephemeral_interaction, success_ephemeral_interaction,
        success_user_interaction,
    },
    ConnType, DB, DBCONNS,
};

use anyhow::Result;
use humantime::format_duration;
use serenity::{
    model::prelude::{
        component::{
            ActionRow, ActionRowComponent,
            ButtonStyle::{Danger, Primary, Secondary, Success},
            InputText,
            InputTextStyle::{Paragraph, Short},
        },
        message_component::MessageComponentInteraction,
        modal::ModalSubmitInteraction,
        ChannelId, GuildChannel,
        InteractionResponseType::Modal,
        ReactionType,
    },
    prelude::Context,
};
use tracing::Instrument;

/// Send a message to the server with prebuilt components for DB channel configuration management
#[instrument(skip(ctx, channel, conn, config))]
pub async fn show(
    ctx: &Context,
    channel: &GuildChannel,
    conn: ConnType,
    config: &Config,
) -> Result<()> {
    let msg = channel.send_message(&ctx, |message| {
        message
        .embed(|embed| {
            embed
            .title("Your SurrealDB session")
            .description(format!("{} \n* You can use `/load` to load a premade dataset or your own SurrealQL from a file.\n* You are using SurrealDB {}.", match conn {
                ConnType::ConnectedChannel => "This channel is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n",
                ConnType::EphemeralChannel => "This brand new channel is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n\n* You can use `/share` to add friends to this channel.",
                ConnType::Thread => "This public thread is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n",
            }, SURREALDB_VERSION.as_str()))
            .footer(|f| {
                f.text(format!("Powered by Surreal Bot {}", BOT_VERSION.as_str())).icon_url("https://cdn.discordapp.com/icons/902568124350599239/cba8276fd365c07499fdc349f55535be.webp?size=240")
            })
            .field("Session lifetime after last query is ", format_duration(config.ttl), true)
            .field("Query timeout is set to ", format_duration(config.timeout), true)
        })
        .components(|c| {
            c.create_action_row(|r| {
                r.create_select_menu(|s| {
                    s.custom_id("configurable_session:format").placeholder("Select output format").min_values(1).max_values(1).options(|o| {
                        o.create_option(|o| o.default_selection(config.json).label("JSON format").value("json"))
                         .create_option(|o| o.default_selection(!config.json).label("SQL-like format").value("sql"))
                    })
                })
            }).create_action_row(|r|{
                r.create_select_menu(|s| {
                    s.custom_id("configurable_session:prettify").placeholder("Prettify output").min_values(1).max_values(1).options(|o| {
                        o.create_option(|o| o.default_selection(config.pretty).label("Pretty output").value("true"))
                         .create_option(|o| o.default_selection(!config.pretty).label("Raw output").value("false"))
                    })
                })
            }).create_action_row(|r|{
                r.create_select_menu(|s| {
                    s.custom_id("configurable_session:require_query").placeholder("Require /query").min_values(1).max_values(1).options(|o| {
                        o.create_option(|o| o.default_selection(!matches!(conn, ConnType::EphemeralChannel)).label("Must use /query or /q").value("true"))
                         .create_option(|o| o.default_selection(matches!(conn, ConnType::EphemeralChannel)).label("All messages are queries").value("false"))
                    })
                })
            }).create_action_row(|mut r| {
                r = r.create_button(|b| b.custom_id("configurable_session:big_query").label("Big Query").style(Primary).emoji('📝'));
                r = r.create_button(|b| b.custom_id("configurable_session:export").label("Export").style(Success).emoji('📃'));
                if matches!(conn, ConnType::Thread) {
                    r = r.create_button(|b| b.custom_id("configurable_session:rename_thread").label("Rename thread").style(Secondary).emoji(TryInto::<ReactionType>::try_into("✏️").expect("Failed to convert emoji")));
                }
                r.create_button(|b| b.custom_id("configurable_session:stop").label("Stop session").style(Danger).emoji('💣'))
            })
        })
    }).await?;

    let ctx_clone = Arc::new(ctx.clone());
    let channel_clone = Arc::new(channel.clone());
    let msg_clone = Arc::new(msg);
    tokio::spawn(
        async move {
            let _ctx = &*ctx_clone;
            match channel_clone.pins(&_ctx).await {
                Ok(pins) => {
                    let old_pin = pins.iter().find(|m| {
                        m.embeds.iter().any(|e| {
                            e.title
                                .as_ref()
                                .map_or(false, |t| t == "Your SurrealDB session")
                        })
                    });

                    if let Some(old_pin) = old_pin {
                        old_pin.unpin(&_ctx).await.unwrap();
                    }
                }
                Err(err) => error!(error = %err, "Failed to get pins"),
            };
            match msg_clone.pin(&_ctx).await {
                Ok(_) => {}
                Err(err) => error!(error = %err, "Failed to pin message"),
            };
        }
        .in_current_span(),
    );

    Ok(())
}

#[instrument(skip(ctx, event))]
pub async fn handle_component(
    ctx: &Context,
    event: &MessageComponentInteraction,
    channel: &ChannelId,
    id: &str,
    values: &[String],
) -> Result<()> {
    let db_exists = DBCONNS.lock().await.get(&channel.0).is_some();

    match (id, db_exists) {
        ("format", true) | ("prettify", true) | ("require_query", true) => {
            debug!("Updating config");
            let mut db = DBCONNS
                .lock()
                .await
                .get_mut(&channel.0)
                .expect("DB disappeared between now above check")
                .clone();
            match id {
                "format" => db.json = values[0] == "json",
                "prettify" => db.pretty = values[0] == "true",
                "require_query" => db.require_query = values[0] == "true",
                _ => unreachable!(),
            }
            DBCONNS.lock().await.insert(channel.0, db);
            ephemeral_interaction(
                ctx,
                event,
                "Config updated",
                format!("{} is now set to {}", id, values[0]),
                Some(true),
            )
            .await?;
        }
        ("export", true) => {
            debug!("Exporting database");
            let conn = DBCONNS
                .lock()
                .await
                .get_mut(&channel.0)
                .expect("DB disappeared between now above check")
                .clone();
            match conn.export_to_attachment().await {
                Ok(Some(attachment)) => {
                    event.create_interaction_response(&ctx, |r| {
                        r.interaction_response_data(|d| {
                            d.embed(|e| {
                                e.title("Exported successfully").description("Find the exported .surql file below.\nYou can either use `/load` and load a new session with it, or use it locally with `surreal import` CLI.").color(0x00ff00)
                            }).add_file(attachment)
                        })
                    }).await?;
                }
                Ok(None) => {
                    CmdError::ExportTooLarge.reply(ctx, event).await?;
                }
                Err(err) => {
                    CmdError::ExportFailed(err).reply(ctx, event).await?;
                }
            }
        }
        ("stop", true) => {
            debug!("Stopping session per user request");
            clean_channel(
                channel
                    .to_channel(&ctx)
                    .await?
                    .guild()
                    .expect("our components are only available in guilds"),
                ctx,
            )
            .await;
        }
        ("big_query", true) | ("copy_big_query", true) => {
            let (query, vars) = if id == "big_query" {
                (Cow::Borrowed(""), Cow::Borrowed(""))
            } else {
                let (mut query, mut vars) = (String::new(), String::new());
                for embed in &event.message.embeds {
                    // TODO: maybe improve this "parsing" of query and vars
                    match embed.title {
                        Some(ref title) if title == BIG_QUERY_SENT_KEY => {
                            query = embed
                                .description
                                .clone()
                                .unwrap_or_default()
                                .replace("```sql\n", "")
                                .replace("\n```", "")
                                .to_string();
                        }
                        Some(ref title) if title == BIG_QUERY_VARS_KEY => {
                            vars = embed
                                .description
                                .clone()
                                .unwrap_or_default()
                                .replace("```json\n", "")
                                .replace("\n```", "")
                                .to_string();
                        }
                        _ => {}
                    }
                }
                (Cow::Owned(query), Cow::Owned(vars))
            };

            event
                .create_interaction_response(&ctx, |a| {
                    a.kind(Modal).interaction_response_data(|d| {
                        d.components(|c| {
                            c.create_action_row(|r| {
                                r.create_input_text(|i| {
                                    i.custom_id("configurable_session:big_query")
                                        .label("Big query")
                                        .style(Paragraph)
                                        .placeholder("Your Surreal query")
                                        .required(true)
                                        .value(query)
                                })
                            })
                            .create_action_row(|r| {
                                r.create_input_text(|i| {
                                    i.custom_id("configurable_session:big_query_variables")
                                        .label("Variables (as JSON)")
                                        .style(Paragraph)
                                        .placeholder("Your Surreal variables (as JSON)")
                                        .required(false)
                                        .value(vars)
                                })
                            })
                        })
                        .custom_id("configurable_session:big_query")
                        .title("Big Query editor")
                    })
                })
                .await?;
        }
        ("reconnect", false) => {
            // TODO: feature creep but maybe a button to re-create a session after it's been deleted
        }
        ("rename_thread", _) => {
            let channel_name = channel
                .to_channel(&ctx)
                .await?
                .guild()
                .expect("our components are only available in guilds")
                .name;
            event
                .create_interaction_response(&ctx, |a| {
                    a.kind(Modal).interaction_response_data(|d| {
                        d.components(|c| {
                            c.create_action_row(|r| {
                                r.create_input_text(|i| {
                                    i.custom_id("configurable_session:rename_thread")
                                        .label("Thread name")
                                        .style(Short)
                                        .value(channel_name)
                                        .placeholder("New thread name")
                                        .min_length(2)
                                        .max_length(100)
                                        .required(true)
                                })
                            })
                        })
                        .custom_id("configurable_session:rename_thread")
                        .title("Rename thread")
                    })
                })
                .await?;
        }
        (_, false) => {
            info!("No connection found for channel");
            CmdError::NoSession.reply(ctx, event).await?;
        }
        _ => {
            warn!(sub_id = id, "Unknown configurable_session component");
        }
    }
    Ok(())
}

#[instrument(skip(ctx, event))]
pub async fn handle_modal(
    ctx: &Context,
    event: &ModalSubmitInteraction,
    channel: &ChannelId,
    id: &str,
    values: &[ActionRow],
) -> Result<()> {
    match id {
        "big_query" => {
            if let ActionRowComponent::InputText(InputText { value, .. }) = &values[0].components[0]
            {
                trace!(raw_query = value, "Received big query");
                match surrealdb::sql::parse(value) {
                    Ok(query) => {
                        debug!(query = ?query, "Parsed big query successfully");
                        let conn = DBCONNS
                            .lock()
                            .await
                            .get_mut(&channel.0)
                            .expect("DB disappeared between now and modal opening")
                            .clone();
                        let vars = match &values[1].components[0] {
                            ActionRowComponent::InputText(InputText { value, .. })
                                if !value.is_empty() =>
                            {
                                match serde_json::from_str(value) {
                                    Ok(vars) => Some(vars),
                                    Err(err) => {
                                        debug!(err = ?err, "Failed to parse variables");
                                        ephemeral_interaction(
                                            ctx,
                                            event,
                                            "Failed to parse big query variables!",
                                            format!("```rust\n{err:#}```"),
                                            Some(false),
                                        )
                                        .await?;
                                        return Ok(());
                                    }
                                }
                            }
                            _ => None,
                        };
                        // Gotta send a response interaction to let modal know we're processing the query
                        ephemeral_interaction(
                            ctx,
                            event,
                            "Big query processing...",
                            "Your query has been sent to the database and is processing now...",
                            Some(true),
                        )
                        .await?;
                        conn.query(ctx, channel, &event.user, query, vars).await?;
                    }
                    Err(err) => {
                        debug!(err = ?err, "Failed to parse big query");
                        return CmdError::BadQuery(err.into()).reply(ctx, event).await;
                    }
                }
            }
        }
        "rename_thread" => {
            if let ActionRowComponent::InputText(InputText { value, .. }) = &values[0].components[0]
            {
                let channel_name = channel
                    .to_channel(&ctx)
                    .await?
                    .guild()
                    .expect("our components are only available in guilds")
                    .name;
                if value == &channel_name {
                    ephemeral_interaction(
                        ctx,
                        event,
                        "Failed to rename thread!",
                        "The new name is the same as the old name.",
                        Some(false),
                    )
                    .await?;
                    return Ok(());
                }
                info!(old_name = %channel_name, new_name = %value, "Renaming thread");
                channel.edit(&ctx, |c| c.name(value)).await?;
                user_interaction(
                    ctx,
                    event,
                    &event.user,
                    "Thread renamed",
                    format!("The thread has been renamed to `{value}`"),
                    Some(true),
                )
                .await?;
            }
        }
        _ => {
            warn!(sub_id = id, "Unknown configurable_session modal");
        }
    }

    Ok(())
}
