use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use paperclip::actix::api_v2_errors;

/// Error type for when only unexpected failures can happen
#[api_v2_errors(code = 500)]
#[derive(Debug)]
pub enum UnexpectedError {
    InternalServerError,
}

impl ResponseError for UnexpectedError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).finish()
    }
}

impl std::fmt::Display for UnexpectedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.status_code().as_str())
    }
}

/// Error type for single item operations
#[api_v2_errors(code = 500, code = 404)]
#[derive(Debug)]
pub enum ShowError {
    InternalServerError,
    NotFound,
}

impl ResponseError for ShowError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).finish()
    }
}

impl std::fmt::Display for ShowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.status_code().as_str())
    }
}

/// Error type for creation operations
#[api_v2_errors(code = 500, code = 400)]
#[derive(Debug)]
pub enum CreateError {
    InternalServerError,
    BadRequest,
}

impl ResponseError for CreateError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest => StatusCode::BAD_REQUEST,
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).finish()
    }
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.status_code().as_str())
    }
}

/// Error type for modification operations
#[api_v2_errors(code = 500, code = 400, code = 404)]
#[derive(Debug)]
pub enum EditError {
    InternalServerError,
    BadRequest,
    NotFound,
}

impl ResponseError for EditError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).finish()
    }
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.status_code().as_str())
    }
}