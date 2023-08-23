use serenity::async_trait;
use serenity::model::channel::Message;

use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::Instant;
use tracing::Instrument;
use tracing::Level;

use crate::commands;
use crate::process;
use crate::utils::ephemeral_interaction;
use crate::utils::respond;
use crate::DBCONNS;

fn validate_msg(msg: &Message) -> bool {
    if msg.author.bot {
        return false;
    };
    true
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        match msg.content.chars().next() {
            Some('#') => return,
            Some('/') => return,
            Some('-') => return,
            None => return,
            _ => {}
        }

        let conn = match DBCONNS.lock().await.get_mut(msg.channel_id.as_u64()) {
            Some(c) => {
                c.last_used = Instant::now();
                if c.require_query {
                    return;
                }
                c.clone()
            }
            None => {
                return;
            }
        };
        if validate_msg(&msg) {
            let result = conn.db.query(&msg.content).await;
            let reply = match process(conn.pretty, conn.json, result) {
                Ok(r) => r,
                Err(e) => e.to_string(),
            };

            respond(reply, ctx, msg.clone(), &conn, msg.channel_id)
                .await
                .unwrap();
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!(user = ?ready.user, "Bot is connected!");

        for guild in ready.guilds {
            let commands =
                GuildId::set_application_commands(&guild.id, &ctx, commands::register_all).await;

            if let Err(why) = commands {
                error!(error = %why, guild_id = %guild.id, "Failed to register commands.");
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {
                let span = span!(
                    Level::DEBUG,
                    "application_command",
                    interaction_id = command.id.0,
                    guild_id = %command.guild_id.unwrap_or_default(),
                    channel_id = %command.channel_id,
                    user = %command.user,
                    command_name = %command.data.name
                );

                async {
                    trace!(command = ?command, "received command interaction");
                    let res = match command.data.name.as_str() {
                        "auth" => commands::auth::run(&command, ctx.clone()).await,
                        "create" => commands::create::run(&command, ctx.clone()).await,
                        "configure" => commands::configure::run(&command, ctx.clone()).await,
                        "share" => commands::share::run(&command, ctx.clone()).await,
                        "create_db_thread" => commands::create_db_thread::run(&command, ctx.clone()).await,
                        "load" => commands::load::run(&command, ctx.clone()).await,
                        "config_update" => commands::config_update::run(&command, ctx.clone()).await,
                        "clean_all" => commands::clean_all::run(&command, ctx.clone()).await,
                        "clean" => commands::clean::run(&command, ctx.clone()).await,
                        "configure_channel" => {
                            commands::configure_channel::run(&command, ctx.clone()).await
                        }
                        "query" => commands::query::run(&command, ctx.clone()).await,
                        "q" => commands::q::run(&command, ctx.clone()).await,
                        "reconnect" => commands::reconnect::run(&command, ctx.clone()).await,
                        "connect" => commands::connect::run(&command, ctx.clone()).await,
                        "export" => commands::export::run(&command, ctx.clone()).await,
                        _ => {
                            warn!(command_name = %command.data.name, command_options = ?command.data.options, "unknown command received");
                            ephemeral_interaction(&ctx, &command, "Unknown command", "Command is currently not implemented", Some(false)).await
                        }
                    };

                    if let Err(why) = res {
                        command
                            .delete_original_interaction_response(&ctx)
                            .await
                            .ok();
                        warn!(error = %why, "Cannot respond to slash command");
                        ephemeral_interaction(&ctx, &command, "Error processing command", format!("There was an error processing this command:\n```rust\n{why}\n```"), Some(false)).await.unwrap()
                    }
                }.instrument(span).await;
            }
            Interaction::MessageComponent(event) => {
                let span = span!(
                    Level::DEBUG,
                    "message_component",
                    interaction_id = event.id.0,
                    guild_id = %event.guild_id.unwrap_or_default(),
                    channel_id = %event.channel_id,
                    user = %event.user,
                    component_id = %event.data.custom_id
                );
                async move {
                    trace!(event = ?event, "received component interaction");
                    let res = match event.data.custom_id.split_once(':') {
                        Some(("configurable_session", id)) => {
                            crate::components::configurable_session::handle_component(
                                &ctx,
                                &event,
                                &event.channel_id,
                                id,
                                &event.data.values,
                            )
                            .await
                        }
                        Some(("configurable_server", id)) => {
                            crate::components::configurable_server::handle_component(
                                &ctx,
                                &event,
                                &event.guild_id.unwrap_or_default(),
                                id,
                                &event.data.values,
                            )
                            .await
                        }
                        _ => Ok(()),
                    };

                    if let Err(why) = res {
                        event.delete_original_interaction_response(&ctx).await.ok();
                        warn!(error = %why, "Failed to process component interaction");
                    }
                }
                .instrument(span)
                .await;
            }
            Interaction::ModalSubmit(modal) => {
                let span = span!(
                    Level::DEBUG,
                    "modal_submit",
                    interaction_id = modal.id.0,
                    guild_id = %modal.guild_id.unwrap_or_default(),
                    channel_id = %modal.channel_id,
                    user = %modal.user,
                    component_id = %modal.data.custom_id
                );
                async move {
                    trace!(modal = ?modal, "received modal submit interaction");
                    let res = match modal.data.custom_id.split_once(':') {
                        Some(("configurable_session", id)) => {
                            crate::components::configurable_session::handle_modal(
                                &ctx,
                                &modal,
                                &modal.channel_id,
                                id,
                                &modal.data.components,
                            )
                            .await
                        }
                        Some(("configurable_server", id)) => {
                            crate::components::configurable_server::handle_modal(
                                &ctx,
                                &modal,
                                &modal.guild_id.unwrap_or_default(),
                                id,
                                &modal.data.components,
                            )
                            .await
                        }
                        _ => Ok(()),
                    };

                    if let Err(why) = res {
                        modal.delete_original_interaction_response(&ctx).await.ok();
                        warn!(error = %why, "Failed to process modal submit interaction");
                    }
                }
                .instrument(span)
                .await;
            }
            _ => {
                warn!("unknown interaction received");
            }
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild) {
        let commands =
            GuildId::set_application_commands(&guild.id, &ctx, commands::register_all).await;

        if let Err(why) = commands {
            error!(error = %why, guild = ?guild, "Failed to register commands.");
        }
    }
}
