use crate::{ConnType, DBCONNS, config::Config, utils::clean_channel, DB};

use serenity::{model::prelude::{GuildChannel, component::{ButtonStyle::{Primary, Danger, Secondary, Success}, ActionRow, InputTextStyle::{Short, Paragraph}, ActionRowComponent, InputText}, ChannelId, message_component::MessageComponentInteraction, InteractionResponseType::Modal, modal::ModalSubmitInteraction, ReactionType}, prelude::Context};
use anyhow::Result;
use humantime::format_duration;

/// Send a message to the server with prebuilt components for DB channel configuration management
pub async fn show_configurable_session(
    ctx: &Context,
    channel: &GuildChannel,
    conn: ConnType,
    config: &Config,
) -> Result<()> {
    let version = DB.version().await?;
    channel.send_message(&ctx, |message| {
        message
        .embed(|embed| {
            embed
            .title("Your SurrealDB session")
            .description(format!("{} \n* You can use `/load` to load a premade dataset or your own SurrealQL from a file.", match conn {
                ConnType::ConnectedChannel => "This channel is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n",
                ConnType::EphemeralChannel => "This brand new channel is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n\n* You can use `/share` to add friends to this channel.",
                ConnType::Thread => "This public thread is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n",
            }))
            .footer(|f| {
                f.text(format!("SurrealDB Version: {}", version)).icon_url("https://cdn.discordapp.com/icons/902568124350599239/cba8276fd365c07499fdc349f55535be.webp?size=240")
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

    Ok(())
}

#[instrument(skip(ctx, event))]
pub async fn handle_session_component(ctx: &Context, event: &MessageComponentInteraction, channel: &ChannelId, id: &str, values: &[String]) -> Result<()> {
    let db_exists = DBCONNS.lock().await.get(&channel.0).is_some();
    
    match (id, db_exists) {
        ("format", true) | ("prettify", true) | ("require_query", true) => {
            debug!("Updating config");
            let mut db = DBCONNS.lock().await.get_mut(&channel.0).unwrap().clone();
            match id {
                "format" => db.json = values[0] == "json",
                "prettify" => db.pretty = values[0] == "true",
                "require_query" => db.require_query = values[0] == "true",
                _ => unreachable!()
            }
            DBCONNS.lock().await.insert(channel.0, db);
            event.create_interaction_response(&ctx, |r| {
                r.interaction_response_data(|d| {
                    d.embed(|e| {
                        e.title("Config updated").description(format!("{} is now set to {}", id, values[0])).color(0x00ff00)
                    }).ephemeral(true)
                })
            }).await?;
        },
        ("export", true) => {
            debug!("Exporting database");
            let channel_name = channel.to_channel(&ctx).await?.guild().unwrap().name;
            let conn = DBCONNS.lock().await.get_mut(&channel.0).unwrap().clone();
            match conn.export(&channel_name).await {
                Ok(Some(path)) => {
                    event.create_interaction_response(&ctx, |r| {
                        r.interaction_response_data(|d| {
                            d.embed(|e| {
                                e.title("Exported successfully").description("Find the exported .surql file below.\nYou can either use `/load` and load a new session with it, or use it locally with `surreal import` CLI.").color(0x00ff00)
                            }).add_file(&path)
                        })
                    }).await?;
                    tokio::fs::remove_file(path).await?;
                },
                Ok(None) => {
                    event.create_interaction_response(&ctx, |r| {
                        r.interaction_response_data(|d| {
                            d.embed(|e| {
                                e.title("Failed to export").description("Export was too big...").color(0xff0000)
                            }).ephemeral(true)
                        })
                    }).await?;
                }
                Err(err) => {
                    event.create_interaction_response(&ctx, |r| {
                        r.interaction_response_data(|d| {
                            d.embed(|e| {
                                e.title("Failed to export").description(format!("{err:#?}")).color(0xff0000)
                            }).ephemeral(true)
                        })
                    }).await?;
                }
            }
        },
        ("stop", true) => {
            debug!("Stopping session per user request");
            clean_channel(channel.to_channel(&ctx).await?.guild().unwrap(), &ctx).await;
        },
        ("big_query", true) => {
            event.create_interaction_response(&ctx, |a| {
                a.kind(Modal).interaction_response_data(|d| {
                    d.components(|c| {
                        c.create_action_row(|r| {
                            r.create_input_text(|i| {
                                i.custom_id("configurable_session:big_query").label("Big query (ignore errors from submit)").style(Paragraph).placeholder("Your Surreal query").required(true)
                            })
                        })
                    }).custom_id("configurable_session:big_query").title("Big query editor")
                })
            }).await?;
        },
        ("reconnect", false) => {
            // TODO: feature creep but maybe a button to re-create a session after it's been deleted
        },
        ("rename_thread", _) => {
            let channel_name = channel.to_channel(&ctx).await?.guild().unwrap().name;
            event.create_interaction_response(&ctx, |a| {
                a.kind(Modal).interaction_response_data(|d| {
                    d.components(|c| {
                        c.create_action_row(|r| {
                            r.create_input_text(|i| {
                                i.custom_id("configurable_session:rename_thread").label("Thread name").style(Short).value(channel_name).placeholder("New thread name").min_length(2).max_length(100).required(true)
                            })
                        })
                    }).custom_id("configurable_session:rename_thread").title("Rename thread")
                })
            }).await?;
        },
        (_, false) => {
            info!("No connection found for channel");
            event.create_interaction_response(&ctx, |a| {
                a.interaction_response_data(|d| {
                    d.embed(|e| {
                        e.title("Session expired or terminated")
                         .description("There is no database instance currently associated with this channel!\nPlease use `/connect` to connect to a new SurrealDB instance.")
                         .color(0xff0000)
                    }).ephemeral(true)
                })
            }).await?;
        }
        _ => {
            warn!(sub_id = id, "Unknown configurable_session component");
        }
    }
    Ok(())
}

#[instrument(skip(ctx, event))]
pub async fn handle_session_modal(ctx: &Context, event: &ModalSubmitInteraction, channel: &ChannelId, id: &str, values: &[ActionRow]) -> Result<()> {
    match id {
        "big_query" => {
            if let ActionRowComponent::InputText(InputText{value, ..}) =  &values[0].components[0] {
                trace!(raw_query = value, "Received big query");
                match surrealdb::sql::parse(&value) {
                    Ok(query) => {
                        debug!(query = ?query, "Parsed big query successfully");
                        let conn = DBCONNS.lock().await.get_mut(&channel.0).unwrap().clone();
                        conn.query(&ctx, channel, &event.user, query).await?;
                    },
                    Err(err) => {
                        debug!(err = ?err, "Failed to parse big query");
                        event.create_interaction_response(&ctx, |r| {
                            r.interaction_response_data(|d| {
                                d.ephemeral(true).embed(|e| {
                                    e.title("Failed to parse query").description(format!("```sql\n{err:#}```")).color(0xff0000)
                                })
                            })
                        }).await?;
                        return Ok(())
                    }
                }
            }
        },
        "rename_thread" => {
            if let ActionRowComponent::InputText(InputText{value, ..}) = &values[0].components[0] {
                let channel_name = channel.to_channel(&ctx).await?.guild().unwrap().name;
                if value == &channel_name {
                    event.create_interaction_response(&ctx, |r| {
                        r.interaction_response_data(|d| {
                            d.ephemeral(true).content("The new name is the same as the old name!")
                        })
                    }).await?;
                    return Ok(())
                }
                info!(old_name = %channel_name, new_name = %value, "Renaming thread");
                channel.edit(&ctx, |c| {
                    c.name(value)
                }).await?;
                event.create_interaction_response(&ctx, |r| {
                    r.interaction_response_data(|d| {
                        d.embed(|e| {
                            e
                            .title("Thread renamed")
                            .description(format!("The thread has been renamed to `{}`", value))
                            .color(0x00ff00)
                            .author(|a| {
                                a.name(&event.user.name).icon_url(event.user.avatar_url().unwrap_or_default())
                            })
                        })
                    })
                }).await?;
            }
        }
        _ => {
            warn!(sub_id = id, "Unknown configurable_session modal");
        }
    }

    Ok(())
}
