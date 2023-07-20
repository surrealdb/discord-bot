use serenity::async_trait;
use serenity::model::channel::Message;

use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::Instant;

use crate::commands;
use crate::process;
use crate::utils::interaction_reply;
use crate::utils::interaction_reply_ephemeral;
use crate::utils::respond;
use crate::DBCONNS;

fn validate_msg(msg: &Message) -> bool {
    if msg.author.bot == true {
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
        println!("{} is connected!", ready.user.name);
        ctx.set_activity(Activity::playing("Making cool things with SurrealDB"))
            .await;

        for guild in ready.guilds {
            let commands =
                GuildId::set_application_commands(&guild.id, &ctx, commands::register_all).await;

            if let Err(why) = commands {
                eprintln!("Failed to register commands: {}", why);
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command);

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
                _ => {
                    interaction_reply(
                        &command,
                        ctx.clone(),
                        "Command is currently not implemented".to_string(),
                    )
                    .await
                }
            };

            if let Err(why) = res {
                println!("Cannot respond to slash command: {}", why);
                interaction_reply_ephemeral(&command, ctx, why)
                    .await
                    .unwrap();
            }
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild) {
        let commands =
            GuildId::set_application_commands(&guild.id, &ctx, commands::register_all).await;

        if let Err(why) = commands {
            eprintln!("Failed to register commands: {}", why);
        }
    }
}
