use crate::{ConnType, DBCONNS, config::Config, utils::respond, process};

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
                    d.ephemeral(true).content(":information_source: Updated session configuration")
                })
            }).await?;
        },
        ("export", true) => {

        },
        ("stop", true) => {

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
                    d.content(":warning: There is no database instance currently associated with this channel\nPlease use `/connect` to connect to a new SurrealDB instance.").ephemeral(true)
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
    info!("Modal interaction received");
    match id {
        "big_query" => {
            if let ActionRowComponent::InputText(InputText{value, ..}) =  &values[0].components[0] {
                match surrealdb::sql::parse(&value) {
                    Ok(query) => {
                        let m = channel.send_message(&ctx, |m| {
                            m.embed(|mut e| {
                                e = e.title("Running query...");
                                e = e.description(format!("```sql\n{query:#}\n```"));
                                e.author(|a| {
                                    a.name(&event.user.name).icon_url(event.user.avatar_url().unwrap_or_default())
                                })
                            })
                        }).await?;
                        let conn = DBCONNS.lock().await.get_mut(&channel.0).unwrap().clone();
                        let result = conn.db.query(query).await;    
                        let reply = match process(conn.pretty, conn.json, result) {
                            Ok(r) => r,
                            Err(e) => e.to_string(),
                        };
                    
                        respond(reply, ctx.clone(), m, &conn, *channel).await?;
                        return Ok(())
                    },
                    Err(e) => {
                        event.create_interaction_response(&ctx, |r| {
                            r.interaction_response_data(|d| {
                                d.ephemeral(true).content(format!(":warning: Failed to parse query:\n```sql\n{}\n```", e))
                            })
                        }).await?;
                        return Ok(())
                    }
                }
            }
        },
        "rename_thread" => {
            if let ActionRowComponent::InputText(InputText{value, ..}) = &values[0].components[0] {

            }
        }
        _ => {
            warn!(sub_id = id, "Unknown configurable_session modal");
        }
    }

    Ok(())
}
