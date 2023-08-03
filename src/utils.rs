use std::{borrow::Cow, cmp::Ordering, path::Path};

use once_cell::sync::Lazy;
use serenity::{
    builder::{CreateInteractionResponse, EditInteractionResponse},
    json::{self, Value},
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction, CommandDataOption, CommandDataOptionValue,
            },
            command::CommandOptionType,
            message_component::MessageComponentInteraction,
            modal::ModalSubmitInteraction,
            AttachmentType, ChannelId, GuildChannel, InteractionId, Message, PermissionOverwrite,
            PermissionOverwriteType,
        },
        user::User,
        Permissions,
    },
    prelude::Context,
};
use surrealdb::{
    engine::local::{Db, Mem},
    opt::auth::Root,
    Surreal,
};
use tokio::time::{sleep_until, Instant};
use tracing::Instrument;

use crate::{config::Config, db_utils::get_config, Conn, ConnType, DBCONNS};

pub const MAX_FILE_SIZE: usize = 24_000_000;

pub enum CmdError {
    NoSubCommand,
    InvalidSubCommand(String),
    InvalidArgument(String, Option<anyhow::Error>),
    ExpectedArgument(String),
    UnexpectedArgumentType(CommandOptionType),
    TooManyArguments(usize, usize),
    NoSession,
    ExpectedNoSession,
    NoGuild,
    NoConfig,
    GetConfig(surrealdb::Error),
    UpdateConfig(surrealdb::Error),
    BuildConfig,
    UnknownDataset(String),
    ExportFailed(anyhow::Error),
    ExportTooLarge,
    BadQuery(surrealdb::Error),
    AttachmentDownload(anyhow::Error),
}

impl CmdError {
    fn message<'a>(&self) -> (Cow<'a, str>, Cow<'a, str>) {
        match self {
            CmdError::NoSubCommand => (
                "Invalid command".into(),
                "Please specify a subcommand".into(),
            ),
            CmdError::InvalidSubCommand(subcommand) => (
                "Invalid command".into(),
                format!("Please specify a valid subcommand.\n`{}` is not a valid subcommand.", subcommand).into(),
            ),
            CmdError::TooManyArguments(expected, got) => (
                "Too many arguments".into(),
                format!("Expected {} arguments, got {}.", expected, got).into(),
            ),
            CmdError::InvalidArgument(argument, maybe_error) => (
                "Invalid argument".into(),
                format!("There was an issue parsing `{}`.{}", argument, match maybe_error {
                    Some(e) => format!(" It returned the following error:\n```rust\n{}\n```", e),
                    None => "".to_string()
                }).into(),
            ),
            CmdError::ExpectedArgument(note) => (
                "Expected an argument".into(),
                format!("Expected an argument, please supply {note}.").into(),
            ),
            CmdError::UnexpectedArgumentType(opt) => (
                "Unexpected argument type".into(),
                format!("Got {opt:?}, this option is not supported for this argument.").into(),
            ),
            CmdError::NoSession => (
                "Session expired or terminated".into(),
                "There is no database instance currently associated with this channel!\nPlease use `/connect` to connect to a new SurrealDB instance.".into()
            ),
            CmdError::ExpectedNoSession => (
                "Session already exists".into(),
                "There is already a database instance associated with this channel!\nPlease use `Stop session` above to stop current SurrealDB instance or use `/config_update` to update current session configuration.".into()
            ),
            CmdError::NoGuild => (
                "Not in a server".into(),
                "Direct messages are not currently supported".into(),
            ),
            CmdError::NoConfig => (
                "Server config not found".into(),
                "No config found for this server, please ask an administrator to configure the bot!".into(),
            ),
            CmdError::GetConfig(e) => (
                "Error while querying for server config".into(),
                format!("Database error:\n```rust\n{e}\n```").into(),
            ),
            CmdError::UpdateConfig(e) => (
                "Error while updating server config".into(),
                format!("Database error:\n```rust\n{e}\n```").into(),
            ),
            CmdError::BuildConfig => (
                "Error while building server config".into(),
                "Please check your config and try again.".into(),
            ),
            CmdError::UnknownDataset(dataset) => (
                "Unknown dataset".into(),
                format!("The dataset `{}` does not exist.", dataset).into(),
            ),
            CmdError::ExportTooLarge => (
                "Export too large".into(),
                "The export is too large to send, sorry.".to_string().into(),
            ),
            CmdError::ExportFailed(e) => (
                "Export failed".into(),
                format!("There was an error while exporting the database:\n```rust\n{e}\n```").into(),
            ),
            CmdError::BadQuery(e) => (
                "Query parse failed".into(),
                format!("There was an error while parsing the query:\n```rust\n{e}\n```").into(),
            ),
            CmdError::AttachmentDownload(e) => (
                "Attachment download failed".into(),
                format!("There was an error while loading the attachment:\n```rust\n{e}\n```").into(),
            ),
        }
    }

    pub async fn reply(
        &self,
        ctx: &Context,
        interaction: impl ToInteraction,
    ) -> Result<(), anyhow::Error> {
        let (title, description) = self.message();
        ephemeral_interaction(ctx, interaction, title, description, Some(false)).await
    }

    pub async fn edit(
        &self,
        ctx: &Context,
        interaction: impl ToInteraction,
    ) -> Result<(), anyhow::Error> {
        let (title, description) = self.message();
        ephemeral_interaction_edit(ctx, interaction, title, description, Some(false)).await
    }
}

/// ToInteraction is a trait that allows for easy conversion of different interaction types to a tuple of the interaction id and token.
pub trait ToInteraction {
    fn to_interaction(&self) -> (&InteractionId, &str);
}

impl ToInteraction for &ApplicationCommandInteraction {
    fn to_interaction(&self) -> (&InteractionId, &str) {
        (&self.id, &self.token)
    }
}

impl ToInteraction for &MessageComponentInteraction {
    fn to_interaction(&self) -> (&InteractionId, &str) {
        (&self.id, &self.token)
    }
}

impl ToInteraction for &ModalSubmitInteraction {
    fn to_interaction(&self) -> (&InteractionId, &str) {
        (&self.id, &self.token)
    }
}

impl ToInteraction for (&InteractionId, &str) {
    fn to_interaction(&self) -> (&InteractionId, &str) {
        ((self.0), (self.1))
    }
}

pub async fn ephemeral_interaction_edit(
    ctx: &Context,
    interaction: impl ToInteraction,
    title: impl ToString,
    description: impl ToString,
    success: Option<bool>,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = EditInteractionResponse::default();
    interaction_response.embed(|e| {
        let e = e
            .title(title.to_string())
            .description(description.to_string());

        match success {
            Some(true) => e.color(0x00ff00),
            Some(false) => e.color(0xff0000),
            None => e,
        }
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    let (_, token) = interaction.to_interaction();
    ctx.http
        .edit_original_interaction_response(token, &Value::from(map))
        .await?;
    Ok(())
}

/// Interaction-source independent function to create an ephemeral interaction message.
pub async fn ephemeral_interaction(
    ctx: &Context,
    interaction: impl ToInteraction,
    title: impl ToString,
    description: impl ToString,
    success: Option<bool>,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = CreateInteractionResponse::default();
    interaction_response.interaction_response_data(|d| {
        d.embed(|e| {
            let e = e
                .title(title.to_string())
                .description(description.to_string());

            match success {
                Some(true) => e.color(0x00ff00),
                Some(false) => e.color(0xff0000),
                None => e,
            }
        })
        .ephemeral(true)
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    let (iid, token) = interaction.to_interaction();
    ctx.http
        .create_interaction_response(iid.0, token, &Value::from(map))
        .await?;
    Ok(())
}

/// Interaction-source independent function to create a logged interaction message for a specific user.
pub async fn user_interaction(
    ctx: &Context,
    interaction: impl ToInteraction,
    user: &User,
    title: impl ToString,
    description: impl ToString,
    success: Option<bool>,
) -> Result<(), anyhow::Error> {
    let mut interaction_response = CreateInteractionResponse::default();
    interaction_response.interaction_response_data(|d| {
        d.embed(|e| {
            let e = e
                .title(title.to_string())
                .description(description.to_string())
                .author(|a| {
                    a.name(&user.name)
                        .icon_url(user.avatar_url().unwrap_or_default())
                });
            match success {
                Some(true) => e.color(0x00ff00),
                Some(false) => e.color(0xff0000),
                None => e,
            }
        })
    });

    let map = json::hashmap_to_json_map(interaction_response.0);
    let (iid, token) = interaction.to_interaction();
    ctx.http
        .create_interaction_response(iid.0, token, &Value::from(map))
        .await?;
    Ok(())
}

/// Interaction-source independent function to create a logged interaction message for system with optional mentions.
pub async fn system_message<'a>(
    ctx: &Context,
    channel: &ChannelId,
    title: impl ToString,
    description: impl ToString,
    success: Option<bool>,
    mentions: Option<String>,
    file: Option<&'a Path>,
) -> Result<(), anyhow::Error> {
    channel.send_message(&ctx, |m| {
        let m = m.embed(|e| {
            let e = e.title(title.to_string())
                .description(description.to_string())
                .author(|a| {
                    a.name("System").icon_url("https://cdn.discordapp.com/icons/902568124350599239/cba8276fd365c07499fdc349f55535be.webp?size=240")
                });
            match success {
                Some(true) => e.color(0x00ff00),
                Some(false) => e.color(0xff0000),
                None => e,
            }
        });
        let m = if let Some(mentions) = mentions {
            m.content(mentions)
        } else {
            m
        };
        if let Some(path) = file {
            m.add_file(path)
        } else {
            m
        }
    }).await?;
    Ok(())
}

pub fn read_view_perms(kind: PermissionOverwriteType) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            .union(Permissions::SEND_MESSAGES)
            .union(Permissions::READ_MESSAGE_HISTORY),
        deny: Permissions::empty(),
        kind,
    }
}

#[instrument(skip_all, fields(guild_id = channel.guild_id.as_u64(), channel_id = channel.id.as_u64(), channel_name = channel.name.clone()))]
pub async fn clean_channel(mut channel: GuildChannel, ctx: &Context) {
    info!("Cleaning up channel");
    let entry = DBCONNS.lock().await.remove(channel.id.as_u64());

    if let Some(conn) = entry {
        match system_message(
            ctx,
            &channel.id,
            "Session expired or terminated",
            "This database instance has expired or was terminated and is no longer functional.",
            Some(false),
            None,
            None,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => error!("Failed to send system message: {}", e),
        };

        match conn.export("expired_session").await {
            Ok(Some(path)) => {
                match system_message(
                    ctx,
                    &channel.id,
                    "Cleanup DB Exported successfully",
                    "You can find your exported DB attached.",
                    Some(true),
                    None,
                    Some(&path),
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send system message: {}", e),
                }
                match tokio::fs::remove_file(path).await {
                    Ok(_) => {}
                    Err(e) => error!("Failed to remove file: {}", e),
                }
            }
            Ok(None) => {
                warn!("Export was too big");
                match system_message(ctx, &channel.id, "Can't upload DB", "Your database is too powerful, it is now gone. (the export was too large to send)", Some(true), None, None).await {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send system message: {}", e),
                }
            }
            Err(err) => {
                error!(error = %err, "Failed to export session");
                match system_message(
                    ctx,
                    &channel.id,
                    "Failed to export",
                    format!("Database export failed:\n```rust\n{err}\n```"),
                    Some(false),
                    None,
                    None,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send system message: {}", e),
                }
            }
        }
    }

    let result = get_config(channel.guild_id).await;

    let response = match result {
        Ok(o) => o,
        Err(_) => return,
    };

    let config = match response {
        Some(c) => c,
        None => return,
    };

    if Some(config.active_channel) == channel.parent_id {
        channel
            .edit(ctx, |c| c.category(config.archive_channel))
            .await
            .ok();
    }

    channel
        .edit_thread(ctx, |thread| {
            thread.archived(true).auto_archive_duration(60)
        })
        .await
        .ok();
}

#[instrument(skip_all)]
pub async fn register_db(
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
    config: Config,
    conn_type: ConnType,
    require_query: bool,
) -> Result<(), anyhow::Error> {
    info!("Registering a new database");
    DBCONNS.lock().await.insert(
        *channel.id.as_u64(),
        crate::Conn {
            db,
            last_used: Instant::now(),
            conn_type,
            ttl: config.ttl,
            pretty: config.pretty,
            json: config.json,
            require_query,
        },
    );

    tokio::spawn(
        async move {
            let mut last_time;
            let mut ttl;
            loop {
                match DBCONNS.lock().await.get(channel.id.as_u64()) {
                    Some(e) => {
                        last_time = e.last_used;
                        ttl = e.ttl
                    }
                    None => {
                        break;
                    }
                }
                if last_time.elapsed() >= ttl {
                    clean_channel(channel, &ctx).await;
                    break;
                }
                sleep_until(last_time + ttl).await;
            }
        }
        .in_current_span(),
    );
    Ok(())
}

pub async fn respond(
    reply: String,
    ctx: Context,
    query_msg: Message,
    conn: &Conn,
    channel_id: ChannelId,
) -> Result<(), anyhow::Error> {
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
        let mut truncated = false;
        let data = match reply.as_bytes().len().cmp(&MAX_FILE_SIZE) {
            Ordering::Equal | Ordering::Less => reply.as_bytes(),
            Ordering::Greater => {
                truncated = true;
                reply.as_bytes().split_at(MAX_FILE_SIZE).0
            }
        };
        let reply_attachment = AttachmentType::Bytes {
            data: std::borrow::Cow::Borrowed(data),
            filename: format!("response.{}", if conn.json { "json" } else { "sql" }),
        };
        channel_id
            .send_message(&ctx, |m| {
                let message = m.reference_message(&query_msg).add_file(reply_attachment);
                if truncated {
                    message.content(
                        ":information_source: Response was too long and has been truncated",
                    )
                } else {
                    message
                }
            })
            .await
            .unwrap();
    }
    Ok(())
}

pub async fn load_attachment(
    op_option: CommandDataOption,
    command: &ApplicationCommandInteraction,
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
) -> Result<(), anyhow::Error> {
    if let Some(CommandDataOptionValue::Attachment(attachment)) = op_option.resolved {
        ephemeral_interaction(
            &ctx,
            command,
            "Downloading your file...",
            format!(
                ":information_source: Your file ({}) is now being downloaded!",
                attachment.filename
            ),
            None,
        )
        .await?;
        match attachment.download().await {
            Ok(data) => {
                ephemeral_interaction_edit(&ctx, command, "Downloaded, importing...", "Your data is currently being loaded, soon you'll be able to query your dataset! \n_Please wait for a confirmation that the dataset is loaded!_", None).await?;
                let db = db.clone();
                let (_channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
                tokio::spawn(
                    async move {
                        if let Err(why) =
                            db.query(String::from_utf8_lossy(&data).into_owned()).await
                        {
                            CmdError::BadQuery(why).edit(&ctx, &command).await.unwrap();
                            return;
                        }
                        ephemeral_interaction_edit(
                            &ctx,
                            &command,
                            "Imported",
                            "Your data is now loaded and ready to query!",
                            Some(true),
                        )
                        .await
                        .unwrap();
                    }
                    .in_current_span(),
                );
                Ok(())
            }
            Err(why) => {
                CmdError::AttachmentDownload(why.into())
                    .edit(&ctx, command)
                    .await
            }
        }
    } else {
        ephemeral_interaction_edit(
            &ctx,
            command,
            "Error with attachment",
            "Unknown error with attachment",
            Some(false),
        )
        .await
    }
}

#[instrument]
pub async fn create_db_instance(server_config: &Config) -> Result<Surreal<Db>, anyhow::Error> {
    info!("Creating database instance");
    let db_config = surrealdb::opt::Config::new()
        .query_timeout(server_config.timeout)
        .transaction_timeout(server_config.timeout);
    let db = Surreal::new::<Mem>((
        db_config,
        Root {
            username: "root",
            password: "root",
        },
    ))
    .await?;

    db.use_ns("test").use_db("test").await?;

    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;

    Ok(db)
}

static LOCK_FILE: &str = include_str!("../Cargo.lock");

pub static SURREALDB_VERSION: Lazy<String> = Lazy::new(|| {
    let lock: cargo_lock::Lockfile = LOCK_FILE.parse().expect("Failed to parse Cargo.lock");
    let package = lock
        .packages
        .iter()
        .find(|p| p.name.as_str() == "surrealdb")
        .expect("Failed to find surrealdb in Cargo.lock");

    match &package.source {
        Some(source) => {
            format!("v{} ({})", package.version, source)
        }
        None => "unknown".to_string(),
    }
});

pub static BOT_VERSION: Lazy<String> = Lazy::new(|| {
    let lock: cargo_lock::Lockfile = LOCK_FILE.parse().expect("Failed to parse Cargo.lock");
    let package = lock
        .packages
        .iter()
        .find(|p| p.name.as_str() == "surreal_bot")
        .expect("Failed to find surreal_bot in Cargo.lock");

    format!("v{}", package.version)
});
