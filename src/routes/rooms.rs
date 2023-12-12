//! Riz API routes for room control

use std::sync::Mutex;

use actix_web::{
    delete,
    error::{ErrorConflict, ErrorNotFound, ErrorServiceUnavailable},
    get, patch, post,
    web::{Data, Json, Path},
    HttpResponse, Responder, Result,
};
use log::error;
use uuid::Uuid;

use crate::{models::Room, storage::Storage, worker::Worker};

/// Create a room
///
/// # Path
///   `POST /v1/rooms`
///
/// # Body
///   [Room]
///
/// # Responses
///   - `200`: [Uuid]
///   - `409`: [String]
///
#[utoipa::path(
    request_body = Room,
    responses(
        (status = 200, description = "OK", body = Uuid),
        (status = 409, description = "Conflict", body = String),
    ),
)]
#[post("/v1/rooms")]
async fn create(req: Json<Room>, storage: Data<Mutex<Storage>>) -> Result<impl Responder> {
    let room = req.into_inner();
    let mut data = storage.lock().unwrap();
    if let Ok(id) = data.new_room(room) {
        Ok(HttpResponse::Ok().json(id))
    } else {
        Err(ErrorConflict("Failed to create new room"))
    }
}

/// Remove a room
///
/// # Path
///   `DELETE /v1/room/{id}`
///
/// # Responses
///   - `204`: [None]
///   - `404`: [String]
///
#[utoipa::path(
    responses(
        (status = 204, description = "OK"),
        (status = 404, description = "Not Found", body = String),
    ),
    params(
        ("id", description = "Room ID")
    )
)]
#[delete("/v1/room/{id}")]
async fn destroy(id: Path<Uuid>, storage: Data<Mutex<Storage>>) -> Result<impl Responder> {
    let id = id.into_inner();
    let mut data = storage.lock().unwrap();
    if let Ok(_) = data.delete_room(&id) {
        Ok(HttpResponse::Ok())
    } else {
        Err(ErrorNotFound(format!("Not found: {}", id)))
    }
}

/// List all room IDs
///
/// # Path
///   `GET /v1/rooms`
///
/// # Responses
///   - `200`: [Vec] of [Uuid]
///   - `404`: [String]
///
#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Uuid>),
        (status = 404, description = "Not Found", body = String),
    ),
)]
#[get("/v1/rooms")]
async fn list(storage: Data<Mutex<Storage>>) -> Result<impl Responder> {
    let data = storage.lock().unwrap();
    if let Ok(ids) = data.list() {
        Ok(HttpResponse::Ok().json(ids))
    } else {
        Err(ErrorNotFound("Failed to list rooms"))
    }
}

/// Read room details
///
/// # Path
///   `GET /v1/room/{id}`
///
/// # Responses
///   - `200`: [Room]
///   - `404`: [String]
///
#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Room),
        (status = 404, description = "Not Found", body = String),
    ),
    params(
        ("id", description = "Room ID")
    )
)]
#[get("/v1/room/{id}")]
async fn read(id: Path<Uuid>, storage: Data<Mutex<Storage>>) -> Result<impl Responder> {
    let id = id.into_inner();
    let data = storage.lock().unwrap();

    if let Some(room) = data.read(&id) {
        Ok(HttpResponse::Ok().json(room))
    } else {
        Err(ErrorNotFound(format!("No such room: {}", id)))
    }
}

/// Update room details
///
/// # Path
///   `PATCH /v1/room/{id}`
///
/// # Body
///   [Room]
///
/// # Responses
///   - `204`: [None]
///   - `404`: [String]
///
#[utoipa::path(
    request_body = Room,
    responses(
        (status = 204, description = "OK"),
        (status = 404, description = "Not Found", body = String),
    ),
    params(
        ("id", description = "Room ID")
    )
)]
#[patch("/v1/room/{id}")]
async fn update(
    id: Path<Uuid>,
    req: Json<Room>,
    storage: Data<Mutex<Storage>>,
) -> Result<impl Responder> {
    let id = id.into_inner();
    let room = req.into_inner();

    let mut data = storage.lock().unwrap();
    if let Ok(_) = data.update_room(&id, &room) {
        Ok(HttpResponse::Ok())
    } else {
        Err(ErrorNotFound(format!("Not found: {}", id)))
    }
}

/// Update lighting status for all bulbs in a room
///
/// # Path
///   `GET /v1/room/{id}/status`
///
/// # Responses
///   - `200`: [Room]
///   - `404`: [String]
///   - `503`: [String]
///
#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Room),
        (status = 404, description = "Not Found", body = String),
        (status = 503, description = "Unavailable", body = String),
    ),
    params(
        ("id", description = "Room ID")
    )
)]
#[get("/v1/room/{id}/status")]
async fn status(
    id: Path<Uuid>,
    data: Data<Mutex<Storage>>,
    worker: Data<Mutex<Worker>>,
) -> Result<impl Responder> {
    let id = id.into_inner();

    let mut room = {
        let data = data.lock().unwrap();
        match data.read(&id) {
            Some(room) => room,
            None => return Err(ErrorNotFound(format!("Not found: {}", id))),
        }
    };

    match room.get_status() {
        Ok(responses) => {
            let mut worker = worker.lock().unwrap();

            for resp in responses {
                match worker.queue_update(resp) {
                    Err(e) => error!("Failed to queue write: {}", e),
                    _ => {}
                };
            }

            Ok(HttpResponse::Ok().json(room))
        }
        Err(e) => Err(ErrorServiceUnavailable(format!(
            "Failed to fetch status: {}",
            e
        ))),
    }
}
