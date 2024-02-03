use std::str::FromStr;
use anyhow::{Context, Result};
use chrono::{LocalResult, NaiveDate, NaiveTime, TimeZone};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client as WebClient;
use sqlx::{prelude::*, Pool, Postgres};
use tokio::sync::Mutex;

pub async fn get_latest_msg(db: Pool<Postgres>) -> Result<Option<(i32, NaiveDate)>> {
    let query = sqlx::query_as(
        "SELECT msg_offset, msg_timestamp::date FROM messages \
        ORDER BY messages.msg_timestamp DESC LIMIT 1",
    );

    Ok(query.fetch_optional(&db).await?)
}

async fn download_logs(web: &WebClient, date: NaiveDate) -> Result<String> {
    let url = format!("https://logs.fomalhaut.me/download/{}.log", date);
    let data = web.get(&url).send().await?.bytes().await?;
    let data = String::from_utf8_lossy(&data).into_owned();
    Ok(data)
}

static MESSAGE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[(\d{2}:\d{2}:\d{2})\] <([^>]+)> (.+)").unwrap());

pub static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

async fn insert_logs(
    db: Pool<Postgres>,
    data: String,
    date: NaiveDate,
    cut_offset: i32,
) -> Result<u64> {
    let mut timestamps = Vec::new();
    let mut offsets = Vec::new();
    let mut authors = Vec::new();
    let mut bodies = Vec::new();

    for (offset, line) in data.lines().enumerate() {
        let offset = offset as i32;
        let c = match MESSAGE_RE.captures(line) {
            Some(v) => v,
            None => continue,
        };

        let time = c.get(1).unwrap().as_str();
        let time = NaiveTime::from_str(time).unwrap();
        let local = chrono_tz::EET.from_local_datetime(&date.and_time(time));
        let timestamp = match local {
            LocalResult::Single(v) => v.naive_utc(),
            _ => {
                println!("Invalid message time, skipping: {} {}", date, time);
                continue;
            }
        };

        if offset <= cut_offset {
            continue;
        }

        timestamps.push(timestamp);
        offsets.push(offset);
        authors.push(c.get(2).unwrap().as_str());
        bodies.push(c.get(3).unwrap().as_str());
    }

    let query =
        sqlx::query("DELETE FROM messages WHERE msg_timestamp::date = $1 AND msg_offset > $2")
            .bind(&date)
            .bind(&cut_offset);
    db.execute(query).await?;

    let query = sqlx::query(
        r#"INSERT INTO messages (msg_timestamp, msg_offset,
            msg_channel, msg_author, msg_body)
        SELECT msg_timestamp, msg_offset,
            $2 AS msg_channel, msg_author, msg_body
        FROM unnest($1::timestamp[], $3::integer[], $4::text[], $5::text[]) AS query(msg_timestamp, msg_offset,
            msg_author, msg_body)"#,
    )
    .bind(timestamps)
    .bind("#cc.ru")
    .bind(offsets)
    .bind(authors)
    .bind(bodies);

    let count = db.execute(query).await?;
    Ok(count.rows_affected())
}

pub async fn download_and_insert_logs(
    db: Pool<Postgres>,
    web: &WebClient,
    date: NaiveDate,
    cut_offset: i32,
) -> Result<u64> {
    let data = download_logs(web, date)
        .await
        .context("failed to download logs")?;

    insert_logs(db, data, date, cut_offset)
        .await
        .context("failed to insert logs")
}