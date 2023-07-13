pub mod configure;
pub mod create;
pub mod create_db_thread;
pub mod load;
pub mod share;

use serenity::builder::CreateApplicationCommands;

pub fn register_all(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    commands
        .create_application_command(|command| create::register(command))
        .create_application_command(|command| configure::register(command))
        .create_application_command(|command| share::register(command))
        .create_application_command(|command| create_db_thread::register(command))
        .create_application_command(|command| load::register(command))
}
