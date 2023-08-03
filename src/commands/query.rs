use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::Context;
use tokio::time::Instant;

use crate::utils::CmdError;
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
        None => return CmdError::NoSession.reply(&ctx, command).await,
    };

    let query = command.data.options.clone()[0]
        .clone()
        .value
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    match surrealdb::sql::parse(&query) {
        Ok(query) => {
            conn.query(
                &ctx,
                &command.channel_id,
                Some(&command),
                &command.user,
                query,
                None,
            )
            .await
        }
        Err(e) => CmdError::BadQuery(e.into()).reply(&ctx, command).await,
    }
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
