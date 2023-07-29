use std::time::Duration;

use serenity::{model::prelude::{GuildChannel, component::ButtonStyle::{Primary, Danger}, ChannelId}, prelude::Context};

use crate::ConnType;
use anyhow::Result;
use humantime::format_duration;

/// Send a message to the server with prebuilt components for DB channel configuration management
pub async fn show_configurable_session(
    ctx: &Context,
    channel: &GuildChannel,
    conn: ConnType,
    ttl: Duration,
) -> Result<()> {
    match conn {
        ConnType::ConnectedChannel => todo!(),
        ConnType::EphemeralChannel => todo!(),
        ConnType::Thread => channel.send_message(&ctx, |message| {
            message
            .embed(|embed| {
                embed
                .title("Your SurrealDB session")
                .description("This public thread is now connected to a SurrealDB instance. \nTry writing some SurrealQL! \n\nIf you want, you can use `/load` to load a premade dataset or your own SurrealQL from a file.")
                .field("Session Lifetime after last query", format_duration(ttl), false)
            })
            .components(|c| { 
                c.create_action_row(|r| {
                    r.create_select_menu(|s| {
                        s.custom_id("configurable_session:format").placeholder("Select output format").min_values(1).max_values(1).options(|o| {
                            o.create_option(|o| o.default_selection(true).label("JSON format").value("json"))
                             .create_option(|o| o.label("SQL-like format").value("sql"))
                        })
                    })
                }).create_action_row(|r|{
                    r.create_select_menu(|s| {
                        s.custom_id("configurable_session:prettify").placeholder("Prettify output").min_values(1).max_values(1).options(|o| {
                            o.create_option(|o| o.default_selection(true).label("Pretty output").value("true"))
                             .create_option(|o| o.label("Raw output").value("false"))
                        })
                    })
                }).create_action_row(|r|{
                    r.create_select_menu(|s| {
                        s.custom_id("configurable_session:require_query").placeholder("Require /query").min_values(1).max_values(1).options(|o| {
                            o.create_option(|o| o.default_selection(true).label("Must use /query or /q").value("true"))
                             .create_option(|o| o.label("All messages are queries").value("false"))
                        })
                    })
                }).create_action_row(|r| {
                    r.create_button(|b| b.custom_id("configurable_session:export").label("Export").style(Primary))
                     .create_button(|b| b.custom_id("configurable_session:stop").label("Stop and cleanup").style(Danger))
                })
            })
        }),
    }
    .await?;

    Ok(())
}

#[instrument(skip(ctx))]
pub async fn handle_session_component(ctx: &Context, channel: &ChannelId, id: &str) -> Result<()> {
    Ok(())
}
