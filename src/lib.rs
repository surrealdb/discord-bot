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
use serenity::model::prelude::{AttachmentType, ChannelId};
use serenity::model::user::User;
use serenity::prelude::Context;
use surrealdb::opt::IntoQuery;
use surrealdb::Error;
use surrealdb::{sql::Value, Response};
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
            Ok(p) => p,
            Err(_) => {
                tokio::fs::create_dir("tmp").await.ok();
                "tmp/".to_string()
            }
        };
        let path = format!("{base_path}{name}.surql");

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
    ) -> Result<(), anyhow::Error> {
        let query_message = channel
            .send_message(&ctx, |m| {
                m.embed(|mut e| {
                    e = e.title("Running query...");
                    e = e.description(format!("```sql\n{query:#}\n```"));
                    e.author(|a| {
                        a.name(&user.name)
                            .icon_url(user.avatar_url().unwrap_or_default())
                    })
                })
            })
            .await?;
        let result = self.db.query(query).await;
        let reply = match process(self.pretty, self.json, result) {
            Ok(r) => r,
            Err(e) => e.to_string(),
        };

        if reply.len() < 4000 {
            channel
                .send_message(&ctx, |m| {
                    m.reference_message(&query_message).embed(|mut e| {
                        e = e.title("Query result");
                        e = e.description(format!(
                            "```{}\n{}\n```",
                            if self.json { "json" } else { "sql" },
                            reply
                        ));
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
                    let message = m
                        .reference_message(&query_message)
                        .add_file(reply_attachment);
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
