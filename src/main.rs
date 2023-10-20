use std::{collections::HashMap, env, error::Error, path::Path, sync::Arc};

pub mod error;
pub mod models;
pub mod config;
mod query;

use chrono::{NaiveDate, Utc};
use config::Config;
use error::{handle_rejection, ErrorResponse};
use futures::executor;
use query::{search, Expr};
use sqlx::{postgres::PgListener, Pool, Postgres};
use tokio::sync::Mutex;
use warp::{
    reject::Rejection,
    reply::{self, WithHeader, WithStatus},
    Filter,
};

use crate::models::Message;

enum QueryOutput {
    PlainText,
    Json,
}

fn query_output_from_string(s: &str) -> Option<QueryOutput> {
    match s {
        "json" => Some(QueryOutput::Json),
        "plaintext" => Some(QueryOutput::PlainText),
        _ => None,
    }
}

fn fmt_database_output(input: Vec<Message>, format: QueryOutput) -> WithStatus<WithHeader<String>> {
    match format {
        QueryOutput::PlainText => {
            let mut output = String::new();

            for message in input {
                output.push_str(&format!(
                    "[{}] <{}> {}\n",
                    message.time.time(),
                    message.author,
                    message.body
                ));
            }

            reply::with_status(
                reply::with_header(output, "Content-Type", "text/plain; charset=utf-8"),
                warp::http::StatusCode::OK,
            )
        }
        QueryOutput::Json => reply::with_status(
            reply::with_header(
                serde_json::to_string(&input).unwrap(),
                "Content-Type",
                "application/json; charset=utf-8",
            ),
            warp::http::StatusCode::OK,
        ),
    }
}

async fn get_log_dates(pool: Pool<Postgres>, dates: Arc<Mutex<Vec<NaiveDate>>>) -> Result<impl warp::Reply, Rejection> {
    let mut lock = dates.lock().await;

    let res = if lock.len() == 0 {
        let result = sqlx::query!(
            "SELECT DISTINCT DATE(date_trunc('day', msg_timestamp)) AS dates FROM messages ORDER BY dates DESC"
        )
        .fetch_all(&pool)
        .await
        .map_err(|_| warp::reject::custom(error::DatabaseError))?;
    
        let res: Vec<NaiveDate> = result
            .iter()
            .filter(|a| a.dates.is_some())
            .map(|f| f.dates.unwrap())
            .collect();

        for date in res.iter() {
            lock.push(date.clone())
        }
        res
    } else {
        lock.to_vec()
    };

    Ok(reply::with_status(
        reply::with_header(
            serde_json::to_string(&res).unwrap(),
            "Content-Type",
            "application/json; charset=utf-8",
        ),
        warp::http::StatusCode::OK,
    ))
}

async fn get_log_by_date(
    path: String,
    params: HashMap<String, String>,
    pool: Pool<Postgres>,
) -> Result<impl warp::Reply, Rejection> {
    let naive_date = NaiveDate::parse_from_str(&path, "%Y-%m-%d");
    let format = query_output_from_string(params.get("format").unwrap_or(&String::new()))
        .unwrap_or(QueryOutput::Json);

    if let Ok(date) = naive_date {
        let result = sqlx::query_as!(
            Message,
            "SELECT msg_body AS body, msg_author AS author, msg_timestamp AS time, msg_offset AS offset FROM messages WHERE DATE(msg_timestamp) = $1",
            date
        )
        .fetch_all(&pool)
        .await.map_err(|_| warp::reject::custom(error::DatabaseError))?;

        Ok(fmt_database_output(result, format))
    } else {
        Err(warp::reject::not_found())
    }
}

async fn get_today_logs(
    params: HashMap<String, String>,
    pool: Pool<Postgres>,
) -> Result<impl warp::Reply, Rejection> {
    let current_datetime = Utc::now();
    let date: NaiveDate = current_datetime.naive_utc().into();
    let format = query_output_from_string(params.get("format").unwrap_or(&String::new()))
        .unwrap_or(QueryOutput::Json);

    let result = sqlx::query_as!(
        Message,
        "SELECT msg_body AS body, msg_author AS author, msg_timestamp AS time, msg_offset AS offset FROM messages WHERE DATE(msg_timestamp) = $1",
        date
    )
    .fetch_all(&pool)
    .await.map_err(|_| warp::reject::custom(error::DatabaseError))?;

    Ok(fmt_database_output(result, format))
}

async fn search_logs(
    params: HashMap<String, String>,
    pool: Pool<Postgres>,
) -> Result<impl warp::Reply, Rejection> {
    let format = query_output_from_string(params.get("format").unwrap_or(&String::new()))
        .unwrap_or(QueryOutput::Json);
    let query = params.get("q").unwrap();

    let expr = Expr::parse(query).map_err(|_| {
        warp::reject::custom(error::ErrorResponse {
            message: String::from("Malformed query"),
            status_code: warp::http::StatusCode::BAD_REQUEST,
        })
    })?;

    let result = search(pool, expr).await.map_err(|err| {
        warp::reject::custom(ErrorResponse {
            message: err.to_string(),
            status_code: warp::http::StatusCode::BAD_REQUEST,
        })
    })?;

    Ok(fmt_database_output(result, format))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let config = Config::new_from_file();
    config.save()?;

    let pool = sqlx::PgPool::connect(&config.postgres_url).await?;
    let mut listener = PgListener::connect(&config.postgres_url).await?;

    listener.listen("chan0").await?;


    let dates = Arc::new(Mutex::new(Vec::new()));

    let timer = timer::Timer::new();
    let dates_timer = dates.clone();

    let _guard = {
        timer.schedule_repeating(chrono::Duration::minutes(5), move || {
            // invalidate cache
            futures::executor::block_on(async {
                let mut data = dates_timer.lock().await;
                data.clear();
            });
        })
    };


    let db_filter = warp::any().map(move || pool.clone());
    let dates_filter = warp::any().map(move || dates.clone());
    let static_files = env::current_dir()?.join(Path::new("static"));

    if static_files.exists() {
        let log_route = warp::path!("logs" / String)
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(get_log_by_date);

        let log_search_route = warp::path!("search")
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(search_logs);

        let log_today_route = warp::path!("logs" / "latest")
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(get_today_logs);

        let logs_total_dates = warp::path!("dates")
            .and(db_filter.clone())
            .and(dates_filter)
            .and_then(get_log_dates);

        warp::serve(
            warp::fs::dir(static_files)
                .or(log_route)
                .or(log_today_route)
                .or(log_search_route)
                .or(logs_total_dates)
                .recover(handle_rejection),
        )
        .run((config.bind_address, config.port))
        .await;
    } else {
        eprintln!(
            "error: cannot find static/ folder, throw static/ folder alongside with executable!"
        );
    }

    Ok(())
}
