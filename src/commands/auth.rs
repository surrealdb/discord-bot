use anyhow::anyhow;
use serde_json::Value;
use serenity::{
    builder::{CreateApplicationCommand, CreateApplicationCommandOption},
    model::prelude::{application_command::CommandDataOption, *},
    prelude::*,
};
use surrealdb::opt::auth::{Database, Namespace, Root, Scope};
use tokio::time::Instant;

use crate::{
    utils::{ephemeral_interaction, user_interaction, CmdError},
    DBCONNS,
};

/// auth
/// - signup
///     - scope <namespace> <database> <scope> <params>
/// - signin
///     - root <username> <password>
///     - ns <namespace> <username> <password>
///     - db <namespace> <database> <username> <password>
///     - scope <namespace> <database> <scope> <params>
/// - token <jwt>
/// - reset (auths as root)
pub async fn run(
    command: &application_command::ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    let res = match command.guild_id {
        Some(_) => {
            let db = match DBCONNS.lock().await.get_mut(command.channel_id.as_u64()) {
                Some(c) => {
                    c.last_used = Instant::now();
                    Ok(c.db.clone())
                }
                None => Err(CmdError::NoSession),
            };

            match (command.data.options.first(), db) {
                (Some(CommandDataOption { name, options, .. }), Ok(db)) => match name.as_str() {
                    "signup" => match options.first() {
                        Some(CommandDataOption { name, options, .. }) => match name.as_str() {
                            "scope" => match scope_options(options) {
                                Ok(scope) => Ok(db.signup(scope).await.map(|_| "SCOPE")),
                                Err(err) => Err(err),
                            },
                            _ => Err(CmdError::InvalidSubCommand(name.to_string())),
                        },
                        None => Err(CmdError::NoSubCommand),
                    },
                    "signin" => match options.first() {
                        Some(CommandDataOption { name, options, .. }) => match name.as_str() {
                            "scope" => match scope_options(options) {
                                Ok(scope) => Ok(db.signin(scope).await.map(|_| "SCOPE")),
                                Err(err) => Err(err),
                            },
                            "db" => match database_options(options) {
                                Ok(scope) => Ok(db.signin(scope).await.map(|_| "DB")),
                                Err(err) => Err(err),
                            },
                            "ns" => match namespace_options(options) {
                                Ok(scope) => Ok(db.signin(scope).await.map(|_| "NS")),
                                Err(err) => Err(err),
                            },
                            "root" => match root_options(options) {
                                Ok(scope) => Ok(db.signin(scope).await.map(|_| "ROOT")),
                                Err(err) => Err(err),
                            },
                            _ => Err(CmdError::InvalidSubCommand(name.to_string())),
                        },
                        None => Err(CmdError::NoSubCommand),
                    },
                    "token" => match string_argument_by_name(options, "jwt") {
                        Ok(token) => Ok(db.authenticate(token).await.map(|_| "JWT")),
                        Err(err) => Err(err),
                    },
                    "reset" => Ok(db
                        .signin(Root {
                            username: "root",
                            password: "root",
                        })
                        .await
                        .map(|_| "ROOT")),
                    _ => Err(CmdError::InvalidSubCommand(name.to_string())),
                },
                (_, Err(e)) => Err(e),
                (None, _) => Err(CmdError::NoSubCommand),
            }
        }
        None => Err(CmdError::NoGuild),
    };

    match res {
        Ok(Ok(new_state)) => {
            user_interaction(
                &ctx,
                command,
                &command.user,
                "Auth successful",
                format!("Session auth changed to `{new_state}`, you can now query under that scope.\nAlternatively you can reset it back root by using `/auth reset`."),
                Some(true),
            )
            .await
        }
        Ok(Err(err)) => {
            ephemeral_interaction(
                &ctx,
                command,
                "Failed to auth",
                format!("Auth method errored:\n```rust\n{}\n```", err),
                Some(false)
            )
            .await
        }
        Err(err) => err.reply(&ctx, command).await,
    }
}

type AuthHashmap = std::collections::HashMap<String, Value>;

fn scope_options(options: &[CommandDataOption]) -> Result<Scope<'_, AuthHashmap>, CmdError> {
    let namespace = string_argument_by_name(options, "namespace")?;
    let database = string_argument_by_name(options, "database")?;
    let scope = string_argument_by_name(options, "scope")?;
    let params = string_argument_by_name(options, "params")?;
    let params = serde_json::from_str::<AuthHashmap>(params).map_err(|e| {
        CmdError::InvalidArgument(
            "params".to_string(),
            Some(anyhow!("failed to parse params as JSON object: {e:?}")),
        )
    })?;

    Ok(Scope {
        namespace,
        database,
        scope,
        params,
    })
}

fn root_options(options: &[CommandDataOption]) -> Result<Root<'_>, CmdError> {
    let username = string_argument_by_name(options, "username")?;
    let password = string_argument_by_name(options, "password")?;

    Ok(Root { username, password })
}

fn namespace_options(options: &[CommandDataOption]) -> Result<Namespace<'_>, CmdError> {
    let namespace = string_argument_by_name(options, "namespace")?;
    let username = string_argument_by_name(options, "username")?;
    let password = string_argument_by_name(options, "password")?;

    Ok(Namespace {
        namespace,
        username,
        password,
    })
}

fn database_options(options: &[CommandDataOption]) -> Result<Database<'_>, CmdError> {
    let namespace = string_argument_by_name(options, "namespace")?;
    let database = string_argument_by_name(options, "database")?;
    let username = string_argument_by_name(options, "username")?;
    let password = string_argument_by_name(options, "password")?;

    Ok(Database {
        namespace,
        database,
        username,
        password,
    })
}

fn string_argument_by_name<'a>(
    options: &'a [CommandDataOption],
    name: &'a str,
) -> Result<&'a str, CmdError> {
    let option = options
        .iter()
        .find(|o| o.name == name)
        .ok_or(CmdError::InvalidArgument(
            name.to_string(),
            Some(anyhow::anyhow!("Argument not found")),
        ))?;

    let value = option.value.as_ref().ok_or(CmdError::InvalidArgument(
        name.to_string(),
        Some(anyhow::anyhow!("Argument has no value")),
    ))?;

    value.as_str().ok_or(CmdError::InvalidArgument(
        name.to_string(),
        Some(anyhow::anyhow!("Value is not a string")),
    ))
}

/// Takes a subcommand name, description and one or multiple options, returns a subcommand
macro_rules! subcommand {
    ($name:expr, $description:expr, $($option:expr),+) => {
        |option|
            option.name($name)
                .description($description)
                .kind(command::CommandOptionType::SubCommand)
                $(.create_sub_option($option))+
    };
}

/// Takes a name and a description, return a string option
macro_rules! string_option (
    ($name:expr, $description:expr) => {
        |option|
            option.name($name)
                .description($description)
                .kind(command::CommandOptionType::String)
                .required(true)
    }
);

fn register_signup(
    option: &mut CreateApplicationCommandOption,
) -> &mut CreateApplicationCommandOption {
    option
        .kind(command::CommandOptionType::SubCommandGroup)
        .name("signup")
        .description("Sign up to a SurrealDB instance")
        .create_sub_option(subcommand! {
            "scope",  "Sign up to a scope",
            string_option!("namespace", "Namespace"),
            string_option!("database", "Database"),
            string_option!("scope", "Scope"),
            string_option!("params", "Additional params (as JSON)")
        })
}

fn register_signin(
    option: &mut CreateApplicationCommandOption,
) -> &mut CreateApplicationCommandOption {
    option
        .kind(command::CommandOptionType::SubCommandGroup)
        .name("signin")
        .description("Sign in to a SurrealDB instance")
        .create_sub_option(subcommand! {
            "root", "Sign in as root",
            string_option!("username", "Username"),
            string_option!("password", "Password")
        })
        .create_sub_option(subcommand! {
            "ns", "Sign in to a namespace",
            string_option!("namespace", "Namespace"),
            string_option!("username", "Username"),
            string_option!("password", "Password")
        })
        .create_sub_option(subcommand! {
            "db", "Sign in to a database",
            string_option!("namespace", "Namespace"),
            string_option!("database", "Database"),
            string_option!("username", "Username"),
            string_option!("password", "Password")
        })
        .create_sub_option(subcommand! {
            "scope", "Sign in to a scope",
            string_option!("namespace", "Namespace"),
            string_option!("database", "Database"),
            string_option!("scope", "Scope"),
            string_option!("params", "Additional params (as JSON)")
        })
}

fn register_token(
    option: &mut CreateApplicationCommandOption,
) -> &mut CreateApplicationCommandOption {
    option
        .kind(command::CommandOptionType::SubCommand)
        .name("token")
        .description("Use a user-defined JSON Web Token to authenticate")
        .create_sub_option(string_option!("jwt", "JSON Web Token"))
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("auth")
        .description("Test authentication configurations for your database")
        .create_option(register_signup)
        .create_option(register_signin)
        .create_option(register_token)
        .create_option(|option| {
            option
                .name("reset")
                .description("Reset your current authentication to default root")
                .kind(command::CommandOptionType::SubCommand)
        })
}
