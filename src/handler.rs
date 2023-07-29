use serenity::async_trait;
use serenity::model::channel::Message;

use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::Instant;
use tracing::Instrument;
use tracing::Level;

use crate::commands;
use crate::process;
use crate::utils::interaction_reply;
use crate::utils::interaction_reply_ephemeral;
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
                        "connect" => commands::connect::run(&command, ctx.clone()).await,
                        "export" => commands::export::run(&command, ctx.clone()).await,
                        _ => {
                            warn!(command_name = %command.data.name, command_options = ?command.data.options, "unknown command received");
                            interaction_reply(
                                &command,
                                ctx.clone(),
                                ":warning: Command is currently not implemented".to_string(),
                            )
                            .await
                        }
                    };

                    if let Err(why) = res {
                        command
                            .delete_original_interaction_response(&ctx)
                            .await
                            .ok();
                        warn!(error = %why, "Cannot respond to slash command");
                        interaction_reply_ephemeral(&command, ctx, ":x: Error processing command".to_string())
                            .await
                            .unwrap();
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
                            crate::components::handle_session_component(
                                &ctx,
                                &event.channel_id,
                                &id,
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
