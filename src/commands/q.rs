use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::CommandOptionType;

use serenity::builder::CreateApplicationCommand;
use serenity::prelude::Context;

use crate::commands;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    commands::query::run(command, ctx).await
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("q")
        .description("alias for query")
        .create_option(|option| {
            option
                .name("query")
                .description("query string to send to the database instance")
                .kind(CommandOptionType::String)
                .required(true)
        })
}
