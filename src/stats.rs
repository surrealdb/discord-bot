use std::{collections::BTreeSet, env, ops::Deref, sync::Arc};

use futures::{stream::FuturesUnordered, StreamExt};
use google_sheets4::{
    hyper_rustls, hyper_util,
    yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey},
    Sheets,
};
use serde_json::json;
use serenity::{
    http::Http,
    model::{
        id::{ChannelId, UserId},
        prelude::ChannelType,
    },
    utils::Guild,
};
use time::OffsetDateTime;
use tokio::time::Instant;

#[derive(Debug)]
pub struct Stats {
    total_members: u64,
    new_members_7days: u64,
    new_members_30days: u64,
    new_forum_posts_7days: u64,
    new_messages_7days: u64,
    new_team_messages_7days: u64,
    new_ambassador_messages_7days: u64,
}

#[derive(Default, Debug)]
struct MessageStats {
    new_7days: u64,
    new_team_7days: u64,
    new_ambassador_7days: u64,
}

impl std::ops::Add for MessageStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            new_7days: self.new_7days + rhs.new_7days,
            new_team_7days: self.new_team_7days + rhs.new_team_7days,
            new_ambassador_7days: self.new_ambassador_7days + rhs.new_ambassador_7days,
        }
    }
}

pub fn start(http: Arc<Http>) {
    tokio::spawn(async move {
        loop {
            chron_midnight().await;
            let start = Instant::now();
            match collect_stats(http.clone()).await {
                Ok(s) => match s.upload().await {
                    Ok(_) => info!(
                        "successfully uploaded stats in {:?}: {s:?}",
                        Instant::now() - start
                    ),
                    Err(e) => error!("error uploading stats: {e}"),
                },
                Err(e) => error!("error generating stats: {e}"),
            }
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

        let _res = hub
            .spreadsheets()
            .values_update(
                serde_json::from_value(json!({"values": [[OffsetDateTime::now_utc().date().to_string(), self.total_members, self.new_members_7days, self.new_members_30days, self.new_forum_posts_7days, self.new_messages_7days, self.new_team_messages_7days, self.new_ambassador_messages_7days]]}))?,
                &sheet_id,
                &format!("Discord!A{}:H{}", lines_filled + 1, lines_filled + 1),
            )
            .value_input_option("USER_ENTERED")
            .doit()
            .await?;

        Ok(())
    }
}

pub async fn collect_stats(http: Arc<Http>) -> Result<Stats, anyhow::Error> {
    // bot should only be in 1 guild
    let guild = http
        .get_guilds(None, Some(1))
        .await?
        .into_iter()
        .next()
        .ok_or(anyhow::Error::msg("not part of any guild"))?;

    let mut total_members = 0;
    let mut new_members_7days = 0;
    let mut new_members_30days = 0;

    let partial_guild = Guild::get(&http, guild.id).await?;
    let team_role = partial_guild
        .role_by_name("SurrealDB")
        .ok_or(anyhow::Error::msg("can't find team role"))?;

    let ambassador_role = partial_guild
        .role_by_name("Surreal Ambassadors")
        .ok_or(anyhow::Error::msg("can't find ambassador role"))?;

    let mut team_members = Vec::new();
    let mut ambassador_members = Vec::new();

    let mut member_iter = guild.id.members_iter(&http).boxed();
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

                if member.roles.contains(&team_role.id) {
                    team_members.push(member.user.id);
                }
                if member.roles.contains(&ambassador_role.id) {
                    ambassador_members.push(member.user.id);
                }
            }
            Err(e) => {
                info!("error processing member: {e:?}");
            }
        }
    }

    let mut new_forum_posts_7days = 0;

    let channels = guild.id.channels(&http).await?;
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

    let active_threads = guild.id.get_active_threads(&http).await?;

    let mut all_channels: BTreeSet<ChannelId> = channels.keys().cloned().collect();
    all_channels.extend(active_threads.threads.iter().map(|c| c.id.clone()));

    for thread in active_threads.threads {
        if let Some(parent) = thread.parent_id {
            if fora.contains(&parent) && (now - *thread.id.created_at()) < time::Duration::days(7) {
                new_forum_posts_7days += 1;
            }
        }
    }

    for forum in fora {
        let threads = forum.get_archived_public_threads(&http, None, None).await?;
        all_channels.extend(threads.threads.iter().map(|c| c.id.clone()));
        for thread in threads.threads {
            if (now - *thread.id.created_at()) < time::Duration::days(7) {
                new_forum_posts_7days += 1;
            }
        }
    }

    let team_members: Arc<[_]> = team_members.into();
    let ambassador_members: Arc<[_]> = ambassador_members.into();

    let mut channel_tasks: FuturesUnordered<_> = all_channels
        .iter()
        .map(|c| {
            tokio::spawn({
                let http = http.clone();
                let team_members = team_members.clone();
                let ambassador_members = ambassador_members.clone();

                collect_channel_message_stats(http, *c, team_members, ambassador_members)
            })
        })
        .collect();

    let mut msg_stats = MessageStats::default();

    while let Some(ms) = channel_tasks.next().await {
        msg_stats = msg_stats + ms?;
    }

    Ok(Stats {
        total_members,
        new_members_7days,
        new_members_30days,
        new_forum_posts_7days,
        new_messages_7days: msg_stats.new_7days,
        new_team_messages_7days: msg_stats.new_team_7days,
        new_ambassador_messages_7days: msg_stats.new_ambassador_7days,
    })
}

async fn collect_channel_message_stats(
    http: Arc<Http>,
    channel_id: ChannelId,
    team_members: impl Deref<Target = [UserId]>,
    ambassador_members: impl Deref<Target = [UserId]>,
) -> MessageStats {
    let mut acc = MessageStats::default();
    let now = OffsetDateTime::now_utc();

    let mut message_stream = channel_id.messages_iter(http).boxed();

    while let Some(msg) = message_stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                info!(?e, "Error accessing message, skipping channel");
                return acc;
            }
        };
        if (now - *msg.timestamp) > time::Duration::days(7) {
            break;
        }
        acc.new_7days += 1;

        if team_members.contains(&msg.author.id) {
            acc.new_team_7days += 1;
        }
        if ambassador_members.contains(&msg.author.id) {
            acc.new_ambassador_7days += 1;
        }
    }

    acc
}
