pub mod configure;
pub mod create;
pub mod share;

use serenity::builder::CreateApplicationCommands;

pub fn register_all(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    commands
        .create_application_command(|command| create::register(command))
        .create_application_command(|command| configure::register(command))
        .create_application_command(|command| share::register(command))
}
