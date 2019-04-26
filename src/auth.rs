use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};

use crate::errors::{ContextualError, ContextualErrorKind};

pub struct Auth;

#[derive(Clone, Debug)]
/// HTTP Basic authentication parameters
pub struct BasicAuthParams {
    pub username: String,
    pub password: String,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
pub fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, ContextualError> {
    let basic_removed = authorization_header
        .to_str()
        .map_err(|e| {
            ContextualError::new(ContextualErrorKind::ParseError(
                "HTTP authentication header".to_string(),
                e.to_string(),
            ))
        })?
        .replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(ContextualErrorKind::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let credentials: Vec<&str> = decoded_str.splitn(2, ':').collect();

    // If argument parsing went fine, it means the HTTP credentials string is well formatted
    // So we can safely unpack the username and the password

    Ok(BasicAuthParams {
        username: credentials[0].to_owned(),
        password: credentials[1].to_owned(),
    })
}

impl Middleware<crate::MiniserveConfig> for Auth {
    fn response(
        &self,
        req: &HttpRequest<crate::MiniserveConfig>,
        resp: HttpResponse,
    ) -> Result<Response> {
        if let Some(ref required_auth) = req.state().auth {
            if let Some(auth_headers) = req.headers().get(header::AUTHORIZATION) {
                let auth_req = match parse_basic_auth(auth_headers) {
                    Ok(auth_req) => auth_req,
                    Err(err) => {
                        let auth_err = ContextualError::new(
                            ContextualErrorKind::HTTPAuthenticationError(Box::new(err)),
                        );
                        return Ok(Response::Done(
                            HttpResponse::BadRequest().body(auth_err.to_string()),
                        ));
                    }
                };
                if auth_req.username != required_auth.username
                    || auth_req.password != required_auth.password
                {
                    let new_resp = HttpResponse::Unauthorized().finish();
                    return Ok(Response::Done(new_resp));
                }
            } else {
                let new_resp = HttpResponse::Unauthorized()
                    .header(
                        header::WWW_AUTHENTICATE,
                        header::HeaderValue::from_static("Basic realm=\"miniserve\""),
                    )
                    .finish();
                return Ok(Response::Done(new_resp));
            }
        }
        Ok(Response::Done(resp))
    }
}
