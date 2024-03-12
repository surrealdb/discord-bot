use std::collections::HashMap;
use std::time::Duration;

use chrono::DateTime;
use chrono::Datelike;
use chrono::TimeDelta;
use chrono::Utc;
use once_cell::sync::Lazy;
use serenity::model::channel::Message;

use serenity::prelude::*;

use tokio::time::Instant;

use crate::process;
use crate::utils::respond;
use crate::DBCONNS;

pub async fn db_msg_handler(ctx: &Context, msg: &Message) -> bool {
    match msg.content.chars().next() {
        Some('#') => return false,
        Some('/') => return false,
        Some('-') => return false,
        None => return false,
        _ => {}
    }

    let conn = match DBCONNS.lock().await.get_mut(msg.channel_id.as_u64()) {
        Some(c) => {
            c.last_used = Instant::now();
            if c.require_query {
                return false;
            }
            c.clone()
        }
        None => {
            return false;
        }
    };
    let result = conn.db.query(&msg.content).await;
    let reply = match process(conn.pretty, conn.json, result) {
        Ok(r) => r,
        Err(e) => e.to_string(),
    };

    respond(reply, ctx.clone(), msg.clone(), &conn, msg.channel_id)
        .await
        .unwrap();
    true
}

pub static OOFMSGS: Lazy<Mutex<HashMap<u64, DateTime<Utc>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn oof_msg_handler(ctx: &Context, msg: &Message) {
    let now = Utc::now();
    let weekday = now.weekday();
    let mut oofmsgs = OOFMSGS.lock().await;

    if let Some(last) = oofmsgs.get(msg.channel_id.as_u64()) {
        if now - last < TimeDelta::try_minutes(10).unwrap() {
            return;
        }
    }
    oofmsgs.insert(msg.channel_id.as_u64().to_owned(), now);
    drop(oofmsgs);

    use chrono::Weekday::*;
    let reply = match weekday {
        Sat | Sun => "It's a weekend!!! We might not be able to get back until Monday.",
        Fri => "Friday!!! We might not be able to get back until monday",
        _ => "It's not working hours, we'll try to get back tomorrow!",
    }
    .to_string();

    let sent_msg = msg.reply(&ctx, reply).await.unwrap();
    let ctx_clone = ctx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        sent_msg.delete(ctx_clone).await.unwrap();
    });
}
