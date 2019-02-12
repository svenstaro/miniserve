use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};

use crate::config;

pub struct Auth;

pub enum BasicAuthError {
    Base64DecodeError,
    InvalidUsernameFormat,
}

#[derive(Clone, Debug)]
pub struct BasicAuthParams {
    pub username: String,
    pub password: String,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
pub fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, BasicAuthError> {
    let basic_removed = authorization_header.to_str().unwrap().replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(|_| BasicAuthError::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let strings: Vec<&str> = decoded_str.splitn(2, ':').collect();
    if strings.len() != 2 {
        return Err(BasicAuthError::InvalidUsernameFormat);
    }
    Ok(BasicAuthParams {
        username: strings[0].to_owned(),
        password: strings[1].to_owned(),
    })
}

impl Middleware<config::MiniserveConfig> for Auth {
    fn response(
        &self,
        req: &HttpRequest<config::MiniserveConfig>,
        resp: HttpResponse,
    ) -> Result<Response> {
        if let Some(ref required_auth) = req.state().auth {
            if let Some(auth_headers) = req.headers().get(header::AUTHORIZATION) {
                let auth_req = match parse_basic_auth(auth_headers) {
                    Ok(auth_req) => auth_req,
                    Err(BasicAuthError::Base64DecodeError) => {
                        return Ok(Response::Done(HttpResponse::BadRequest().body(format!(
                            "Error decoding basic auth base64: '{}'",
                            auth_headers.to_str().unwrap()
                        ))));
                    }
                    Err(BasicAuthError::InvalidUsernameFormat) => {
                        return Ok(Response::Done(
                            HttpResponse::BadRequest().body("Invalid basic auth format"),
                        ));
                    }
                };
                if auth_req.username != required_auth.username
                    || auth_req.password != required_auth.password
                {
                    let new_resp = HttpResponse::Forbidden().finish();
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
