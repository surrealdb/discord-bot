use serenity::async_trait;
use serenity::model::channel::Message;

use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::Instant;

use crate::commands;
use crate::process;
use crate::utils::interaction_reply;
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
        if let Some(conn) = DBCONNS.lock().await.get_mut(msg.channel_id.as_u64()) {
            conn.last_used = Instant::now();
            let result = conn.db.query(&msg.content).await;
            if validate_msg(&msg) {
                let reply = match process(conn.pretty, conn.json, result) {
                    Ok(r) => r,
                    Err(e) => e.to_string(),
                };

                if reply.len() < 1900 {
                    msg.reply(
                        &ctx,
                        format!(
                            "```{}\n{}\n```",
                            if conn.json { "json" } else { "sql" },
                            reply
                        ),
                    )
                    .await
                    .unwrap();
                } else {
                    let reply_attachment = AttachmentType::Bytes {
                        data: std::borrow::Cow::Borrowed(reply.as_bytes()),
                        filename: format!("response.{}", if conn.json { "json" } else { "sql" }),
                    };
                    msg.channel_id
                        .send_message(&ctx, |m| {
                            m.reference_message(&msg).add_file(reply_attachment)
                        })
                        .await
                        .unwrap();
                }
            }
        } else {
            return;
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
                _ => {
                    interaction_reply(
                        &command,
                        ctx.clone(),
                        "Command is curretnly not implemented".to_string(),
                    )
                    .await
                }
            };

            if let Err(why) = res {
                println!("Cannot respond to slash command: {}", why);
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
