use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::AttachmentType;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::Context;
use tokio::time::Instant;

use crate::process;

use crate::utils::{interaction_reply, interaction_reply_ephemeral};
use crate::DBCONNS;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if let Some(conn) = DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
        conn.last_used = Instant::now();

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

        if reply.len() < 1900 {
            query_msg
                .reply(
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
            command
                .channel_id
                .send_message(&ctx, |m| {
                    m.reference_message(&query_msg).add_file(reply_attachment)
                })
                .await
                .unwrap();
        }
    } else {
        interaction_reply_ephemeral(command, ctx, "No database instance found for this channel")
            .await?;
    }
    Ok(())
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("query")
        .description("query database instance")
        .create_option(|option| {
            option
                .name("query")
                .description("query string to send to the database instance")
                .kind(CommandOptionType::String)
                .required(true)
        })
}
