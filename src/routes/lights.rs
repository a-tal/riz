//! Riz API routes for light control

use std::sync::Mutex;

use actix_web::{
    delete,
    error::{ErrorConflict, ErrorNotFound, ErrorServiceUnavailable},
    get, patch, post, put,
    web::{Data, Json, Path},
    HttpResponse, Responder, Result,
};
use log::error;
use uuid::Uuid;

use crate::{
    models::{Light, LightRequest, LightingResponse},
    storage::Storage,
    worker::Worker,
};

/// Create a light
///
/// # Path
///   `POST /v1/room/{id}/lights`
///
/// # Body
///   [Light]
///
/// # Responses
///   - `200`: [Uuid]
///   - `409`: [String]
///
#[utoipa::path(
    request_body = Light,
    responses(
        (status = 200, description = "OK", body = Uuid),
        (status = 409, description = "Conflict", body = String),
    ),
    params(
        ("id", description = "Room ID")
    )
)]
#[post("/v1/room/{id}/lights")]
async fn create(
    id: Path<Uuid>,
    req: Json<Light>,
    storage: Data<Mutex<Storage>>,
) -> Result<impl Responder> {
    let id = id.into_inner();
    let light = req.into_inner();
    let mut data = storage.lock().unwrap();
    if let Ok(id) = data.new_light(&id, light) {
        Ok(HttpResponse::Ok().json(id))
    } else {
        Err(ErrorConflict("Failed to create new light"))
    }
}

/// Update lighting settings for all bulbs in a room
///
/// # Path
///   `PUT /v1/room/{id}/lights`
///
/// # Body
///   [LightRequest]
///
/// # Responses
///   - `204`: [None]
///   - `404`: [String]
///   - `503`: [String]
///
#[utoipa::path(
    request_body = LightRequest,
    responses(
        (status = 204, description = "OK"),
        (status = 404, description = "Not Found", body = String),
        (status = 503, description = "Unavailable", body = String),
    ),
    params(
        ("id", description = "Room ID"),
    ),
)]
#[put("/v1/room/{id}/lights")]
async fn update_room(
    id: Path<Uuid>,
    req: Json<LightRequest>,
    storage: Data<Mutex<Storage>>,
    worker: Data<Mutex<Worker>>,
) -> Result<impl Responder> {
    let id = id.into_inner();
    let req = req.into_inner();

    let room = {
        let data = storage.lock().unwrap();
        match data.read(&id) {
            Some(room) => room,
            None => return Err(ErrorNotFound(format!("No such room: {}", id))),
        }
    };

    if let Some(lights) = room.list() {
        let mut worker = worker.lock().unwrap();
        for light_id in lights {
            if let Some(light) = room.read(light_id) {
                if let Err(_) = worker.create_task(light.ip(), req.clone()) {
                    return Err(ErrorServiceUnavailable(format!("No available workers")));
                }
            }
        }
        Ok(HttpResponse::Ok())
    } else {
        Err(ErrorNotFound(format!("No lights in room: {}", id)))
    }
}

/// Update lighting settings for a single bulb
///
/// # Path
///   `PUT /v1/room/{id}/light/{light_id}`
///
/// # Body
///   [LightRequest]
///
/// # Responses
///   - `204`: [None]
///   - `404`: [String]
///   - `503`: [String]
///
#[utoipa::path(
    request_body = LightRequest,
    responses(
        (status = 204, description = "OK"),
        (status = 404, description = "Not Found", body = String),
        (status = 503, description = "Unavailable", body = String),
    ),
    params(
        ("id", description = "Room ID"),
        ("light_id", description = "Light ID"),
    )
)]
#[put("/v1/room/{id}/light/{light_id}")]
async fn update(
    ids: Path<(Uuid, Uuid)>,
    req: Json<LightRequest>,
    storage: Data<Mutex<Storage>>,
    worker: Data<Mutex<Worker>>,
) -> Result<impl Responder> {
    let (room_id, light_id) = ids.into_inner();
    let req = req.into_inner();

    let room = {
        let data = storage.lock().unwrap();
        match data.read(&room_id) {
            Some(room) => room,
            None => return Err(ErrorNotFound(format!("No such room: {}", room_id))),
        }
    };

    if let Some(light) = room.read(&light_id) {
        let mut worker = worker.lock().unwrap();
        match worker.create_task(light.ip(), req) {
            Ok(_) => Ok(HttpResponse::Ok()),
            Err(_) => Err(ErrorServiceUnavailable(format!("No available workers"))),
        }
    } else {
        Err(ErrorNotFound(format!("No such light: {}", light_id)))
    }
}

/// Update lighting status for a single bulb
///
/// # Path
///   `GET /v1/room/{id}/light/{light_id}/status`
///
/// # Responses
///   - `200`: [crate::models::LightStatus]
///   - `404`: [String]
///   - `503`: [String]
///
#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = LightStatus),
        (status = 404, description = "Not Found", body = String),
        (status = 503, description = "Unavailable", body = String),
    ),
    params(
        ("id", description = "Room ID"),
        ("light_id", description = "Light ID"),
    )
)]
#[get("/v1/room/{id}/light/{light_id}/status")]
async fn status(
    ids: Path<(Uuid, Uuid)>,
    data: Data<Mutex<Storage>>,
    worker: Data<Mutex<Worker>>,
) -> Result<impl Responder> {
    let (room_id, light_id) = ids.into_inner();

    let room = {
        let data = data.lock().unwrap();
        match data.read(&room_id) {
            Some(room) => room,
            None => return Err(ErrorNotFound(format!("No such room: {}", room_id))),
        }
    };

    if let Some(light) = room.read(&light_id) {
        match light.get_status() {
            Ok(status) => {
                let mut worker = worker.lock().unwrap();
                match worker.queue_update(LightingResponse::status(light.ip(), status.clone())) {
                    Err(e) => error!("Failed to queue write: {}", e),
                    _ => {}
                };
                Ok(HttpResponse::Ok().json(status))
            }
            Err(e) => Err(ErrorServiceUnavailable(format!(
                "Failed to fetch status: {}",
                e
            ))),
        }
    } else {
        Err(ErrorNotFound(format!("No such light: {}", light_id)))
    }
}

/// Update light details
///
/// # Path
///   `PATCH /v1/room/{id}/light/{light_id}`
///
/// # Body
///   [Light]
///
/// # Responses
///   - `204`: [None]
///   - `404`: [String]
///
#[utoipa::path(
    request_body = Light,
    responses(
        (status = 204, description = "OK"),
        (status = 404, description = "Not Found", body = String),
    ),
    params(
        ("id", description = "Room ID"),
        ("light_id", description = "Light ID"),
    )
)]
#[patch("/v1/room/{id}/light/{light_id}")]
async fn update_light(
    ids: Path<(Uuid, Uuid)>,
    light: Json<Light>,
    storage: Data<Mutex<Storage>>,
) -> Result<impl Responder> {
    let (room_id, light_id) = ids.into_inner();
    let light = light.into_inner();

    let mut data = storage.lock().unwrap();
    if let Ok(_) = data.update_light(&room_id, &light_id, &light) {
        Ok(HttpResponse::Ok())
    } else {
        Err(ErrorNotFound(format!("Not found: {}", room_id)))
    }
}

/// Remove a light
///
/// # Path
///   `DELETE /v1/room/{id}/light/{light_id}`
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
        ("id", description = "Room ID"),
        ("light_id", description = "Light ID")
    )
)]
#[delete("/v1/room/{id}/light/{light_id}")]
async fn destroy(ids: Path<(Uuid, Uuid)>, storage: Data<Mutex<Storage>>) -> Result<impl Responder> {
    let (room_id, light_id) = ids.into_inner();
    let mut data = storage.lock().unwrap();
    if let Ok(_) = data.delete_light(&room_id, &light_id) {
        Ok(HttpResponse::Ok())
    } else {
        Err(ErrorNotFound(format!(
            "Not found: {} in room {}",
            light_id, room_id
        )))
    }
}
