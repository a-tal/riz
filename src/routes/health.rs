//! Riz API health route

use actix_web::{get, HttpResponse, Responder, Result};

/// Simple ping route
///
/// # Path
///   `GET /v1/ping`
///
/// # Responses
///   - `200`: [String]
///
#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = String),
    ),
)]
#[get("/v1/ping")]
pub async fn ping() -> Result<impl Responder> {
    // could check if we are having any issues opening sockets...
    Ok(HttpResponse::Ok().json("ok"))
}
