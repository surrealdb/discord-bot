use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::Context;
use tokio::time::Instant;

use crate::process;

use crate::utils::{interaction_reply, interaction_reply_ephemeral, respond};
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let conn = match DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
        Some(c) => {
            c.last_used = Instant::now();
            c.clone()
        }
        None => {
            interaction_reply_ephemeral(
                command,
                ctx,
                ":warning: No database instance found for this channel",
            )
            .await?;
            return Ok(());
        }
    };

    let query = command.data.options.clone()[0]
        .clone()
        .value
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    interaction_reply(command, ctx.clone(), &query).await?;

    let query_msg = command.get_interaction_response(&ctx).await?;

    let result = conn.db.query(query).await;
    let reply = match process(conn.pretty, conn.json, result) {
        Ok(r) => r,
        Err(e) => e.to_string(),
    };

    respond(reply, ctx, query_msg, &conn, command.channel_id).await
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("query")
        .description("Query SurrealDB")
        .create_option(|option| {
            option
                .name("query")
                .description("Query string to send to SurrealDB")
                .kind(CommandOptionType::String)
                .required(true)
        })
}
