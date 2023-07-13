pub mod channel_info;
pub mod commands;
pub mod config;
pub mod db_utils;
pub mod hander;
pub mod premade;
pub mod utils;

use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use surrealdb::Error;
use surrealdb::{sql::Value, Response};
use tokio::time::{Duration, Instant};

use std::collections::HashMap;

use serenity::prelude::*;

use once_cell::sync::Lazy;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

pub static DBCONNS: Lazy<Mutex<HashMap<u64, Conn>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub static DB: Surreal<Db> = Surreal::init();

#[derive(Debug)]
pub struct Conn {
    db: Surreal<Db>,
    last_used: Instant,
    conn_type: ConnType,
    ttl: Duration,
    pretty: bool,
    json: bool,
}

#[derive(Debug)]
pub enum ConnType {
    Channel,
    Thread,
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
