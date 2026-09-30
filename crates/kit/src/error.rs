//! Errors an action can return, and the status Rails would answer with.
//!
//! Rails maps exception classes to statuses in `ActionDispatch::ExceptionWrapper.rescue_responses`
//! and renders `public/<status>.html` through `ActionDispatch::PublicExceptions`. [`Error::Halt`]
//! is not an error at all: it's how a before-action that rendered or redirected stops the chain.

use axum::http::StatusCode;

use crate::response::Response;

pub type Result<T = Response, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A before-action (or the action itself) produced the response; stop and send it.
    #[error("halted with {}", .0.status)]
    Halt(Box<Response>),

    /// `ActionController::BadRequest`, `ParamError`s, `Parameters::ParseError`.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// `ActionController::ParameterMissing`.
    #[error("param is missing or the value is empty or invalid: {0}")]
    ParameterMissing(String),
    /// `ActionController::InvalidAuthenticityToken` (the `:exception` forgery strategy).
    #[error("invalid authenticity token: {0}")]
    InvalidAuthenticityToken(String),
    /// `ActionController::InvalidCrossOriginRequest`.
    #[error("invalid cross-origin request")]
    InvalidCrossOriginRequest,
    /// `ActionController::UnknownFormat` / `MissingExactTemplate`.
    #[error("unknown format")]
    UnknownFormat,
    /// `ActiveRecord::RecordNotFound`, `ActionController::RoutingError`.
    #[error("not found")]
    NotFound,
    /// `ActionController::UnknownHttpMethod`.
    #[error("unknown http method")]
    MethodNotAllowed,
    /// `ActionDispatch::Cookies::CookieOverflow` (not rescued by Rails: a 500).
    #[error("{0} cookie overflowed")]
    CookieOverflow(String),
    /// `ActionController::Redirecting::UnsafeRedirectError` and friends (a 500).
    #[error("unsafe redirect: {0}")]
    UnsafeRedirect(String),
    /// `ActionDispatch::RemoteIp::IpSpoofAttackError` (a 500).
    #[error("IP spoofing attack")]
    IpSpoofAttack,
    /// Any other status an app wants to map an error to.
    #[error("{0}")]
    Status(StatusCode),
    /// Like [`Error::Status`], keeping the error that caused it for the log line (see
    /// [`Error::with_status`]).
    #[error("{0}: {1:#}")]
    WithStatus(StatusCode, anyhow::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Error::Halt(response) => response.status,
            Error::BadRequest(_) | Error::ParameterMissing(_) => StatusCode::BAD_REQUEST,
            Error::InvalidAuthenticityToken(_) | Error::InvalidCrossOriginRequest => StatusCode::UNPROCESSABLE_ENTITY,
            Error::UnknownFormat => StatusCode::NOT_ACCEPTABLE,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Error::Status(status) | Error::WithStatus(status, _) => *status,
            Error::CookieOverflow(_) | Error::UnsafeRedirect(_) | Error::IpSpoofAttack | Error::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn internal(error: impl Into<anyhow::Error>) -> Self {
        Error::Internal(error.into())
    }

    /// Answer `error` with `status` instead of a 500.
    pub fn with_status(status: StatusCode, error: impl Into<anyhow::Error>) -> Self {
        Error::WithStatus(status, error.into())
    }
}

impl From<crate::params::ParamError> for Error {
    fn from(error: crate::params::ParamError) -> Self {
        Error::BadRequest(error.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Internal(error.into())
    }
}

/// Stop the before-action chain with `response`, like a Rails callback that renders or redirects.
///
/// ```ignore
/// fn ensure_can_administer(c: &mut Ctx) -> Result<()> {
///     if !can_administer(c) { return halt(c.head(StatusCode::FORBIDDEN)); }
///     Ok(())
/// }
/// ```
pub fn halt<T>(response: Response) -> Result<T> {
    Err(Error::Halt(Box::new(response)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_status_answers_the_status_and_logs_the_cause() {
        let cause = anyhow::anyhow!("name can't be blank").context("validation failed");
        let error = Error::with_status(StatusCode::UNPROCESSABLE_ENTITY, cause);

        assert_eq!(error.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(format!("{error:#}"), "422 Unprocessable Entity: validation failed: name can't be blank");
    }
}
