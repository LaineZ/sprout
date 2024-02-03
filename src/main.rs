#![allow(dead_code)]

use std::{
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    env,
    error::Error,
    path::Path,
    str::FromStr,
    sync::Arc,
    time::Instant,
};

mod config;
mod error;
mod import;
mod models;
mod query;

use chrono::{NaiveDate, Utc};
use config::Config;
use error::{AnyhowError, ErrorResponse};
use handlebars::Handlebars;
use query::{search, Expr};
use reqwest::Client as WebClient;
use serde::Serialize;
use serde_json::json;
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

struct WithTemplate<T: Serialize> {
    name: &'static str,
    value: T,
}

fn query_output_from_string(s: &str) -> Option<QueryOutput> {
    match s {
        "json" => Some(QueryOutput::Json),
        "plaintext" => Some(QueryOutput::PlainText),
        _ => None,
    }
}

fn render<T>(template: WithTemplate<T>, hbs: Arc<Handlebars<'_>>) -> impl warp::Reply
where
    T: Serialize,
{
    let render = hbs
        .render(template.name, &template.value)
        .unwrap_or_else(|err| err.to_string());
    warp::reply::html(render)
}

fn with_template_engine(
    hb: Arc<Handlebars<'_>>,
) -> impl Filter<Extract = (Arc<Handlebars<'_>>,), Error = Infallible> + Clone {
    warp::any().map(move || hb.clone())
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

async fn get_log_dates(
    pool: Pool<Postgres>,
    dates: Arc<Mutex<Vec<NaiveDate>>>,
) -> Result<impl warp::Reply, Rejection> {
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
            "SELECT msg_id AS id, msg_body AS body, msg_author AS author, msg_timestamp AS time, msg_offset AS offset FROM messages WHERE DATE(msg_timestamp) = $1",
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
        "SELECT msg_id AS id, msg_body AS body, msg_author AS author, msg_timestamp AS time, msg_offset AS offset FROM messages WHERE DATE(msg_timestamp) = $1",
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

fn html_error<E: ToString>(error: E) -> WithTemplate<serde_json::Value> {
    WithTemplate {
        name: "search.html",
        value: json!({ "error": error.to_string() }),
    }
}

pub async fn view_search_as_html(
    params: HashMap<String, String>,
    hb: Arc<Handlebars<'_>>,
    pool: Pool<Postgres>,
) -> Result<impl warp::Reply, Rejection> {
    let query = params.get("q");
    if query.is_none() {
        return Ok(render(
            html_error("Search parameter is missing in URL"),
            hb.clone(),
        ));
    }

    let query = query.unwrap();
    if query.is_empty() {
        return Ok(render(html_error("Empty expression string"), hb.clone()));
    }

    let expression = Expr::parse(query);

    match expression {
        Ok(expr) => match search(pool, expr).await {
            Ok(messages) => {
                let mut message_groups: BTreeMap<NaiveDate, Vec<Message>> = BTreeMap::new();

                for message in &messages {
                    let date = message.time.date();

                    message_groups
                        .entry(date)
                        .or_insert(vec![])
                        .push(message.clone());
                }

                let mut message_results: Vec<models::MessageResults> = message_groups
                    .iter()
                    .map(|(date, messages)| models::MessageResults {
                        date: date.clone(),
                        messages: messages
                            .iter()
                            .map(|x| models::MessageTemplate::from(x.clone()))
                            .collect(),
                    })
                    .collect();

                message_results.reverse();

                let template = if !message_results.is_empty() {
                    WithTemplate {
                        name: "search.html",
                        value: json!({ "messages": message_results }),
                    }
                } else {
                    html_error("No results")
                };

                Ok(render(template, hb.clone()))
            }
            Err(err) => Ok(render(html_error(err), hb.clone())),
        },
        Err(err) => Ok(render(html_error(err), hb.clone())),
    }
}

pub async fn view_log_as_html(
    path: String,
    hb: Arc<Handlebars<'_>>,
    pool: Pool<Postgres>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let date = NaiveDate::parse_from_str(&path, "%Y-%m-%d").unwrap_or({
        let current_datetime = Utc::now();
        current_datetime.naive_utc().into()
    });

    let result = sqlx::query_as!(
        models::MessageTemplate,
        "SELECT msg_id AS id, msg_body AS body, msg_author AS author, msg_timestamp::time AS time, msg_offset AS offset FROM messages WHERE DATE(msg_timestamp) = $1",
        date
    )
    .fetch_all(&pool)
    .await.map_err(|_| warp::reject::custom(error::DatabaseError))?;

    let template = if !result.is_empty() {
        WithTemplate {
            name: "index.html",
            value: json!({ "messages": result }),
        }
    } else {
        html_error(format!("No results for date: {}", date))
    };

    Ok(render(template, hb.clone()))
}

pub async fn import(date: String, db: Pool<Postgres>) -> Result<impl warp::Reply, warp::Rejection> {
    let _guard = match import::LOCK.try_lock() {
        Ok(v) => v,
        Err(_) => {
            return Err(warp::reject::custom(ErrorResponse {
                message: "Import already running".to_string(),
                status_code: warp::http::StatusCode::BAD_REQUEST,
            }))
        }
    };

    let today = Utc::now().naive_utc();
    let mut cut_offset = -1;

    let start = if !date.is_empty() {
        NaiveDate::from_str(&date).map_err(|op| AnyhowError(op.into()))?
    } else {
        let (o, d) = import::get_latest_msg(db.clone())
            .await
            .map_err(|op| AnyhowError(op.into()))?
            .ok_or(anyhow::anyhow!("cannot get start date"))
            .map_err(|op| AnyhowError(op.into()))?;
        cut_offset = o;
        d
    };

    let web = WebClient::new();
    let instant = Instant::now();
    let mut date = start;
    let mut count = 0;

    while date <= today.into() {
        count += import::download_and_insert_logs(db.clone(), &web, date, cut_offset)
            .await
            .map_err(|op| AnyhowError(op.into()))?;
        cut_offset = -1;
        date = date.succ_opt().unwrap();
    }

    let json = json!({
        "count": count,
        "elapsed_time": instant.elapsed().as_millis()
    });

    Ok(reply::with_status(
        reply::with_header(
            json.to_string(),
            "Content-Type",
            "application/json; charset=utf-8",
        ),
        warp::http::StatusCode::OK,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new_from_file();
    config.save()?;

    let pool = sqlx::PgPool::connect(&config.postgres_url).await?;
    let mut listener = PgListener::connect(&config.postgres_url).await?;

    listener.listen("chan0").await?;

    let mut hb = Handlebars::new();
    hb.register_template_file("index.html", "template/index.handlebars")?;
    hb.register_template_file("search.html", "template/search.handlebars")?;

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
    let hb = Arc::new(hb);

    if static_files.exists() {
        let log_route = warp::path!("logs" / String)
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(get_log_by_date);

        let log_search_route = warp::path!("logs" / "search")
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(search_logs);

        let log_today_route = warp::path!("logs" / "latest")
            .and(warp::query::<HashMap<String, String>>())
            .and(db_filter.clone())
            .and_then(get_today_logs);

        let log_total_dates = warp::path!("dates")
            .and(db_filter.clone())
            .and(dates_filter)
            .and_then(get_log_dates);

        let log_import_by_date = warp::path!("import" / String)
            .and(db_filter.clone())
            .and_then(import);

        let log_import = warp::path!("import")
            .and(db_filter.clone())
            .and_then(move |db: Pool<Postgres>| {
                import(String::new(), db)
            });

        let log_interface = warp::path!(String)
            .and_then(|segment: String| async move {
                if segment != "search" {
                    Ok(segment)
                } else {
                    Err(warp::reject::not_found())
                }
            })
            .and(with_template_engine(hb.clone()))
            .and(db_filter.clone())
            .and_then(view_log_as_html);

        let log_interface_index = warp::path::end()
            .and(warp::any().map(|| String::new()))
            .and(with_template_engine(hb.clone()))
            .and(db_filter.clone())
            .and_then(view_log_as_html);

        let log_interface_search = warp::path!("search")
            .and(warp::query::<HashMap<String, String>>())
            .and(with_template_engine(hb.clone()))
            .and(db_filter.clone())
            .and_then(view_search_as_html);

        warp::serve(
            warp::fs::dir(static_files)
                .or(log_import)
                .or(log_import_by_date)
                .or(log_interface_index)
                .or(log_route)
                .or(log_today_route)
                .or(log_search_route)
                .or(log_total_dates)
                .or(log_interface)
                .or(log_interface_search)
                .recover(error::handle_rejection_json),
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
