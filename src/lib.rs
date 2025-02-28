pub mod channel_info;
pub mod commands;
pub mod components;
pub mod config;
pub mod db_utils;
pub mod handler;
pub mod premade;
pub mod stats;
pub mod utils;

use futures::StreamExt;
use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use serenity::{
    builder::CreateEmbed,
    http::Http,
    model::{
        prelude::{
            application_command::ApplicationCommandInteraction, component::ButtonStyle::Primary,
            Attachment, AttachmentType, ChannelId,
        },
        user::User,
    },
    prelude::Context,
};
use surrealdb::{opt::IntoQuery, sql, Error, Response};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use utils::{ephemeral_interaction_edit, CmdError, ToInteraction, MAX_FILE_SIZE};

#[macro_use]
extern crate tracing;

use std::collections::HashMap;
use std::{cmp::Ordering, sync::LazyLock};

use surrealdb::engine::local::Db;
use surrealdb::Surreal;

pub static DBCONNS: LazyLock<Mutex<HashMap<u64, Conn>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub static DB: LazyLock<Surreal<Db>> = LazyLock::new(Surreal::init);

pub const BIG_QUERY_SENT_KEY: &str = "Query sent";
pub const BIG_QUERY_VARS_KEY: &str = "Variables sent";

#[derive(Debug, Clone)]
pub struct Conn {
    db: Surreal<Db>,
    last_used: Instant,
    conn_type: ConnType,
    ttl: Duration,
    pretty: bool,
    json: bool,
    require_query: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnType {
    ConnectedChannel,
    EphemeralChannel,
    Thread,
}

impl Conn {
    pub async fn import_from_attachment(
        &self,
        http: impl AsRef<Http>,
        i: impl ToInteraction,
        attachment: &Attachment,
    ) -> Result<(), anyhow::Error> {
        ephemeral_interaction_edit(
            &http,
            i.clone(),
            "Downloading attachment",
            format!("Now downloading `{}`, please wait.", attachment.filename),
            None,
        )
        .await?;
        match attachment.download().await {
            Ok(bytes) => {
                ephemeral_interaction_edit(&http, i.clone(), "Downloaded, now importing...", "Your data is currently being loaded, soon you'll be able to query your dataset! \n_Please wait for a confirmation that the dataset is loaded!_", None).await?;
                match self
                    .db
                    .query(String::from_utf8_lossy(&bytes).into_owned())
                    .await
                {
                    Ok(_) => {
                        ephemeral_interaction_edit(http, i, "Imported successfully!", "Your data has been imported successfully!\nYou can now query your dataset.", Some(true)).await?;
                        Ok(())
                    }
                    Err(why) => {
                        CmdError::BadQuery(why).edit(http, i).await?;
                        Ok(())
                    }
                }
            }
            Err(err) => {
                CmdError::AttachmentDownload(err.into())
                    .edit(http, i)
                    .await?;
                Ok(())
            }
        }
    }

    #[must_use]
    pub async fn export_to_attachment(&self) -> Result<Option<AttachmentType>, anyhow::Error> {
        let mut acc = Vec::new();

        let mut export_stream = self.db.export(()).await?;
        while let Some(v) = export_stream.next().await {
            acc.extend(v?);
        }

        let reply_attachment = AttachmentType::Bytes {
            data: std::borrow::Cow::Owned(acc),
            filename: "export.surql".to_string(),
        };
        Ok(Some(reply_attachment))
    }

    pub async fn query(
        &self,
        ctx: &Context,
        channel: &ChannelId,
        interaction: Option<&ApplicationCommandInteraction>,
        user: &User,
        query: impl IntoQuery + std::fmt::Display,
        vars: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(), anyhow::Error> {
        let query_message = match interaction {
            Some(i) => {
                i.create_interaction_response(ctx, |r| {
                    r.interaction_response_data(|m| {
                        m.embed(|mut e| {
                            e = e.title(BIG_QUERY_SENT_KEY);
                            e = e.description(format!("```sql\n{query:#}\n```"));
                            e.author(|a| {
                                a.name(&user.name)
                                    .icon_url(user.avatar_url().unwrap_or_default())
                            })
                        })
                        .components(|c| {
                            c.create_action_row(|r| {
                                r.create_button(|b| {
                                    b.custom_id("configurable_session:big_query")
                                        .label("New Big Query please")
                                        .style(Primary)
                                        .emoji('📝')
                                })
                                .create_button(|b| {
                                    b.custom_id("configurable_session:copy_big_query")
                                        .label("Copy this Big Query")
                                        .style(Primary)
                                        .emoji('🔁')
                                })
                            })
                        });
                        if let Some(vars) = &vars {
                            m.add_embed({
                                let mut e = CreateEmbed::default();
                                e.title(BIG_QUERY_VARS_KEY);
                                e.description(format!(
                                    "```json\n{:#}\n```",
                                    serde_json::to_string_pretty(&vars).unwrap_or_default()
                                ));
                                e
                            })
                        } else {
                            m
                        }
                    })
                })
                .await?;
                i.get_interaction_response(ctx).await?
            }
            None => {
                channel
                    .send_message(&ctx, |mut m| {
                        m = m
                            .embed(|mut e| {
                                e = e.title(BIG_QUERY_SENT_KEY);
                                e = e.description(format!("```sql\n{query:#}\n```"));
                                e.author(|a| {
                                    a.name(&user.name)
                                        .icon_url(user.avatar_url().unwrap_or_default())
                                })
                            })
                            .components(|c| {
                                c.create_action_row(|r| {
                                    r.create_button(|b| {
                                        b.custom_id("configurable_session:big_query")
                                            .label("New Big Query please")
                                            .style(Primary)
                                            .emoji('📝')
                                    })
                                    .create_button(|b| {
                                        b.custom_id("configurable_session:copy_big_query")
                                            .label("Copy this Big Query")
                                            .style(Primary)
                                            .emoji('🔁')
                                    })
                                })
                            });
                        if let Some(vars) = &vars {
                            m.add_embed(|mut e| {
                                e = e.title(BIG_QUERY_VARS_KEY);
                                e = e.description(format!(
                                    "```json\n{:#}\n```",
                                    serde_json::to_string_pretty(&vars).unwrap_or_default()
                                ));
                                e
                            })
                        } else {
                            m
                        }
                    })
                    .await?
            }
        };
        let mut query = self.db.query(query);
        if let Some(vars) = vars {
            query = query.bind(vars);
        }
        let now = std::time::Instant::now();
        let result = query.await;
        let elapsed = now.elapsed();
        let reply = match process(self.pretty, self.json, result) {
            Ok(r) => r,
            Err(e) => e.to_string(),
        };

        if reply.len() < 4000 {
            channel
                .send_message(&ctx, |m| {
                    m.reference_message(&query_message).embed(|mut e| {
                        e = e.title("Query result");
                        e = e
                            .description(format!(
                                "```{}\n{}\n```",
                                if self.json { "json" } else { "sql" },
                                reply
                            ))
                            .field("Query took", humantime::format_duration(elapsed), true);
                        e.author(|a| {
                            a.name(&user.name)
                                .icon_url(user.avatar_url().unwrap_or_default())
                        })
                    })
                })
                .await?;
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
                filename: format!("response.{}", if self.json { "json" } else { "sql" }),
            };
            channel
                .send_message(&ctx, |m| {
                    m
                        .reference_message(&query_message)
                        .add_file(reply_attachment).embed(|mut e| {
                            e = e.title("Query result").field("Query took", humantime::format_duration(elapsed), true);
                            return if truncated {
                                e.description(
                                    ":information_source: Response was too long and has been truncated",
                                )
                            } else {
                                e
                            }
                        })
                })
                .await
                .unwrap();
        }
        Ok(())
    }
}

/// Exports all DBCONNS to their respective channels and returns.
/// Used as part of graceful shutdown.
pub async fn shutdown(http: impl AsRef<Http>) -> Result<(), anyhow::Error> {
    let mut errors = vec![];
    for (c, conn) in DBCONNS.lock().await.iter() {
        let channel = ChannelId::from(*c);
        match conn.export_to_attachment().await {
            Ok(Some(attchment)) => {
                let res = channel.send_message(&http, |m| {
                    m.embed(|e| {
                        e.title("Pre-shutdown DB Exported successfully").description("Sorry! The bot had to go offline for maintenance, your session has been exported. You can find the .surql file attached.\nYou can either use `/reconnect` and load a new session with it when the bot is back online, or use it locally with `surreal import` CLI.").color(0x00ff00)
                    }).add_file(attchment).components(|c| c.create_action_row(|r| r.create_button(|b| b.label("Reconnect").custom_id("configurable_session:reconnect").style(Primary).emoji('📦'))))
                }).await;
                if let Err(why) = res {
                    errors.push(why.to_string())
                };
            }
            Ok(None) => {
                warn!("Export was too big")
            }
            Err(err) => {
                error!(error = %err, "Failed to export session");
            }
        }
    }
    DBCONNS.lock().await.clear();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::Error::msg(errors.join(",")))
    }
}

pub fn process(
    pretty: bool,
    json: bool,
    res: surrealdb::Result<Response>,
) -> Result<String, Error> {
    // Check query response for an error
    let mut response = res?;
    // Get the number of statements the query contained
    let num_statements = response.num_statements();
    // Prepare a single value from the query response
    let value = if num_statements > 1 {
        let mut output = Vec::<sql::Value>::with_capacity(num_statements);
        for index in 0..num_statements {
            output.push(match response.take::<surrealdb::Value>(index) {
                Ok(v) => v.into_inner(),
                Err(e) => sql::Value::from(e.to_string()),
            });
        }
        sql::Value::from(output)
    } else {
        response.take::<surrealdb::Value>(0)?.into_inner()
    };
    // Check if we should emit JSON and/or prettify
    Ok(match (json, pretty) {
        // Don't prettify the SurrealQL response
        (false, false) => value.to_string(),
        // Yes prettify the SurrealQL response
        (false, true) => format!("{value:#}"),
        // Don't pretty print the JSON response
        (true, false) => {
            // panic!();
            serde_json::to_string(&value.into_json()).unwrap()
        }
        // Yes prettify the JSON response
        (true, true) => {
            // panic!();
            let mut buf = Vec::new();
            let mut serializer = serde_json::Serializer::with_formatter(
                &mut buf,
                PrettyFormatter::with_indent(b"\t"),
            );
            value.into_json().serialize(&mut serializer).unwrap();
            String::from_utf8(buf).unwrap()
        }
    })
}
