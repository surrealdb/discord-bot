use std::{env, sync::Arc};

use futures::StreamExt;
use google_sheets4::{
    hyper_rustls, hyper_util,
    yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey},
    Sheets,
};
use serde_json::json;
use serenity::{
    http::Http,
    model::{id::ChannelId, prelude::ChannelType},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug)]
pub struct Stats {
    total_members: u64,
    new_members_7days: u64,
    new_members_30days: u64,
    new_forum_posts_7days: u64,
}

pub fn start(http: Arc<Http>) {
    tokio::spawn(async move {
        loop {
            match collect_stats(&http).await {
                Ok(s) => match s.upload().await {
                    Ok(_) => info!("successfully uploaded stats: {s:?}"),
                    Err(e) => error!("error uploading stats: {e}"),
                },
                Err(e) => error!("error generating stats: {e}"),
            }
            chron_midnight().await;
        }
    });
}

async fn chron_midnight() {
    let now = OffsetDateTime::now_utc();
    let next_time = OffsetDateTime::new_utc(now.date().next_day().unwrap(), time::Time::MIDNIGHT);
    let time_til_next = next_time - now;

    tokio::time::sleep(time_til_next.try_into().unwrap()).await;
}

impl Stats {
    pub async fn upload(&self) -> Result<(), anyhow::Error> {
        let secret = env::var("DRIVE_SECRET")?;
        let account_key: ServiceAccountKey = serde_json::from_str(&secret).unwrap();
        let sheet_id = env::var("SHEET_ID")?;

        let client1 =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(
                    hyper_rustls::HttpsConnectorBuilder::new()
                        .with_native_roots()
                        .unwrap()
                        .https_or_http()
                        .enable_http1()
                        .build(),
                );

        let auth = ServiceAccountAuthenticator::with_client(account_key, client1)
            .build()
            .await
            .unwrap();

        let client2 =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(
                    hyper_rustls::HttpsConnectorBuilder::new()
                        .with_native_roots()
                        .unwrap()
                        .https_or_http()
                        .enable_http1()
                        .build(),
                );

        let hub = Sheets::new(client2, auth);

        let res = hub
            .spreadsheets()
            .values_get(&sheet_id, "Discord!A:A")
            .doit()
            .await?;

        let lines_filled = res.1.values.unwrap().len();

        let mut now = Vec::new();
        OffsetDateTime::now_utc().format_into(&mut now, &Rfc3339)?;
        let now = String::from_utf8(now)?;

        let _res = hub
            .spreadsheets()
            .values_update(
                serde_json::from_value(json!({"values": [[now, self.total_members, self.new_members_7days, self.new_members_30days, self.new_forum_posts_7days]]}))?,
                &sheet_id,
                &format!("Discord!A{}:E{}", lines_filled + 1, lines_filled + 1),
            )
            .value_input_option("USER_ENTERED")
            .doit()
            .await?;

        Ok(())
    }
}

pub async fn collect_stats(http: impl AsRef<Http>) -> Result<Stats, anyhow::Error> {
    let http = http.as_ref();
    // bot should only be in 1 guild
    let guild = http
        .get_guilds(None, Some(1))
        .await?
        .into_iter()
        .next()
        .ok_or(anyhow::Error::msg("not part of any guild"))?;

    info!("got guild: {guild:?}");

    let mut total_members = 0;
    let mut new_members_7days = 0;
    let mut new_members_30days = 0;

    let mut member_iter = guild.id.members_iter(http).boxed();
    let now = OffsetDateTime::now_utc();

    while let Some(member) = member_iter.next().await {
        match member {
            Ok(member) => {
                total_members += 1;

                if let Some(join_time) = member.joined_at {
                    let joined_duration = now - *join_time;

                    if joined_duration < time::Duration::days(7) {
                        new_members_7days += 1;
                    }

                    if joined_duration < time::Duration::days(30) {
                        new_members_30days += 1;
                    }
                }
            }
            Err(e) => {
                info!("error processing member: {e:?}");
            }
        }
    }

    let mut new_forum_posts_7days = 0;

    let channels = guild.id.channels(http).await?;
    let fora: Vec<ChannelId> = channels
        .values()
        .filter_map(|c| {
            if c.kind == ChannelType::Forum {
                Some(c.id)
            } else {
                None
            }
        })
        .collect();

    let threads = guild.id.get_active_threads(http).await?;

    for thread in threads.threads {
        if let Some(parent) = thread.parent_id {
            if fora.contains(&parent) && (now - *thread.id.created_at()) < time::Duration::days(7) {
                new_forum_posts_7days += 1;
            }
        }
    }

    for forum in fora {
        let threads = forum.get_archived_public_threads(http, None, None).await?;
        for thread in threads.threads {
            if (now - *thread.id.created_at()) < time::Duration::days(7) {
                new_forum_posts_7days += 1;
            }
        }
    }

    Ok(Stats {
        total_members,
        new_members_7days,
        new_members_30days,
        new_forum_posts_7days,
    })
}
