use std::{convert::Infallible, error::Error};
use serde::Serialize;
use warp::{http::StatusCode, Rejection, Reply, reject};

#[derive(Serialize)]
pub struct ErrorMessage {
    pub message: String,
}

#[derive(Debug)]
pub struct ErrorResponse {
    pub message: String,
    pub status_code: StatusCode,
}

#[derive(Debug)]
pub struct DatabaseError;

impl reject::Reject for DatabaseError {}
impl reject::Reject for ErrorResponse {}


pub async fn handle_rejection_json(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        message = "NOT_FOUND";
        code = StatusCode::NOT_FOUND;
    } else if let Some(DatabaseError) = err.find() {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Database error";
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        message = "METHOD_NOT_ALLOWED";
        code = StatusCode::METHOD_NOT_ALLOWED;
    } else if let Some(error) = err.find::<ErrorResponse>() {
        message = &error.message;
        code = error.status_code;
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        message = "UNHANDLED_REJECTION";
        code = StatusCode::INTERNAL_SERVER_ERROR;
    }

    let json = warp::reply::json(&ErrorMessage {
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}