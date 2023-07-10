use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::prelude::command::Command;
use serenity::model::prelude::*;
use serenity::prelude::*;

use tokio::time::{sleep, sleep_until, Duration, Instant};

use surrealdb::engine::local::Mem;
use surrealdb::Surreal;

use crate::commands;
use crate::process;
use crate::DB;
use crate::{DBCONNS, DEFAULT_TTL};

fn validate_msg(msg: &Message) -> bool {
    if msg.author.bot == true {
        return false;
    };
    true
}

async fn clean_channel(channel: GuildChannel, ctx: &Context) {
    let _ = channel
        .say(
            &ctx,
            "This database instance has expired and is no longer functional",
        )
        .await;

    DBCONNS.lock().await.remove(channel.id.as_u64());
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Some(conn) = DBCONNS.lock().await.get_mut(msg.channel_id.as_u64()) {
            conn.last_used = Instant::now();
            let result = conn.db.query(&msg.content).await;
            if validate_msg(&msg) {
                let reply = match process(true, true, result) {
                    Ok(r) => r,
                    Err(e) => e.to_string(),
                };
                msg.reply(&ctx, reply).await.unwrap();
            }
        } else {
            return;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        for guild in ready.guilds {
            let commands = GuildId::set_application_commands(&guild.id, &ctx, |commands| {
                commands.create_application_command(|command| commands::create::register(command))
            })
            .await;

            if let Err(why) = commands {
                eprintln!("Failed to register commands: {}", why);
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                "create" => commands::create::run(&command, ctx.clone()).await,
                _ => "Command is curretnly not implemented".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                println!("Cannot respond to slash command: {}", why);
            }
        }
    }
}
