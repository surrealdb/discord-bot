pub mod clean;
pub mod clean_all;
pub mod config_update;
pub mod configure;
pub mod configure_channel;
pub mod connect;
pub mod create;
pub mod create_db_thread;
pub mod load;
pub mod q;
pub mod query;
pub mod share;

use serenity::builder::CreateApplicationCommands;

pub fn register_all(commands: &mut CreateApplicationCommands) -> &mut CreateApplicationCommands {
    commands
        .create_application_command(|command| create::register(command))
        .create_application_command(|command| configure::register(command))
        .create_application_command(|command| share::register(command))
        .create_application_command(|command| create_db_thread::register(command))
        .create_application_command(|command| load::register(command))
        .create_application_command(|command| config_update::register(command))
        .create_application_command(|command| clean_all::register(command))
        .create_application_command(|command| clean::register(command))
        .create_application_command(|command| configure_channel::register(command))
        .create_application_command(|command| query::register(command))
        .create_application_command(|command| q::register(command))
        .create_application_command(|command| connect::register(command))
}
