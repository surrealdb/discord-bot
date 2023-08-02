pub mod channel_info;
pub mod commands;
pub mod components;
pub mod config;
pub mod db_utils;
pub mod handler;
pub mod premade;
pub mod utils;

use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use serenity::{
    model::{
        prelude::{component::ButtonStyle::Primary, AttachmentType, ChannelId},
        user::User,
    },
    prelude::Context, http::Http,
};
use surrealdb::{opt::IntoQuery, sql::Value, Error, Response};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use utils::MAX_FILE_SIZE;

#[macro_use]
extern crate tracing;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

pub static DBCONNS: Lazy<Mutex<HashMap<u64, Conn>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub static DB: Surreal<Db> = Surreal::init();

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

#[derive(Debug, Clone)]
pub enum ConnType {
    ConnectedChannel,
    EphemeralChannel,
    Thread,
}

impl Conn {
    #[must_use]
    pub async fn export(&self, name: &str) -> Result<Option<PathBuf>, anyhow::Error> {
        let base_path = match std::env::var("TEMP_DIR_PATH") {
            Ok(p) => PathBuf::from(p),
            Err(_) => {
                tokio::fs::create_dir("tmp").await.ok();
                PathBuf::from("tmp/")
            }
        };
        let path = base_path.join(name).with_extension("surql");

        match self.db.export(&path).await {
            Ok(()) => match tokio::fs::metadata(&path).await {
                Ok(metadata) => {
                    if metadata.len() < utils::MAX_FILE_SIZE as u64 {
                        Ok(Some(Path::new(&path).to_owned()))
                    } else {
                        tokio::fs::remove_file(path).await?;
                        Ok(None)
                    }
                }
                Err(err) => Err(err.into()),
            },
            Err(why) => Err(why.into()),
        }
    }

    pub async fn query(
        &self,
        ctx: &Context,
        channel: &ChannelId,
        user: &User,
        query: impl IntoQuery + std::fmt::Display,
        vars: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(), anyhow::Error> {
        let query_message = channel
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
                            }).create_button(|b| {
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
            .await?;
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
    for (c, conn) in DBCONNS.lock().await.iter() {
        let channel = ChannelId::from(*c);
        match conn.export("shutdown_export").await {
            Ok(Some(path)) => {
                channel.send_message(&http, |m| {
                    m.embed(|e| {
                        e.title("Pre-shutdown DB Exported successfully").description("Find the exported .surql file below.\nYou can either use `/reconnect` and load a new session with it, or use it locally with `surreal import` CLI.").color(0x00ff00)
                    }).add_file(&path)
                }).await?;
                tokio::fs::remove_file(path).await?;
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
    Ok(())
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
        let mut output = Vec::<Value>::with_capacity(num_statements);
        for index in 0..num_statements {
            output.push(match response.take(index) {
                Ok(v) => v,
                Err(e) => e.to_string().into(),
            });
        }
        Value::from(output)
    } else {
        response.take(0)?
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
