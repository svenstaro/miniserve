use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};
use sha2::{Sha256, Sha512, Digest};

pub struct Auth;

/// HTTP Basic authentication errors
pub enum BasicAuthError {
    Base64DecodeError,
}

#[derive(Clone, Debug)]
/// HTTP Basic authentication parameters
pub struct BasicAuthParams {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub enum RequiredAuthPassword {
    Plain(String),
    Sha256(String),
    Sha512(String),
}

#[derive(Clone, Debug)]
/// Authentication structure to match BasicAuthParams against
pub struct RequiredAuth {
    pub username: String,
    pub password: RequiredAuthPassword,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
pub fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, BasicAuthError> {
    let basic_removed = authorization_header.to_str().unwrap().replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(|_| BasicAuthError::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let credentials: Vec<&str> = decoded_str.splitn(2, ':').collect();

    // If argument parsing went fine, it means the HTTP credentials string is well formatted
    // So we can safely unpack the username and the password

    Ok(BasicAuthParams {
        username: credentials[0].to_owned(),
        password: credentials[1].to_owned(),
    })
}

pub fn match_auth(basic_auth: BasicAuthParams, required_auth: &RequiredAuth) -> bool {
    if basic_auth.username != required_auth.username {
        return false;
    }

    match &required_auth.password {
        RequiredAuthPassword::Plain(ref required_password) => basic_auth.password == *required_password,
        RequiredAuthPassword::Sha256(password_hash) => compare_hash::<Sha256>(basic_auth.password, password_hash),
        RequiredAuthPassword::Sha512(password_hash) => compare_hash::<Sha512>(basic_auth.password, password_hash),
    }
}

pub fn compare_hash<T: Digest>(password: String, hash: &String) -> bool {
    get_hash_hex::<T>(password) == *hash
}

fn get_hash_hex<T: Digest>(text: String) -> String {
    let mut hasher = T::new();
    hasher.input(text);
    hex::encode(hasher.result())
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
                    Err(BasicAuthError::Base64DecodeError) => {
                        return Ok(Response::Done(HttpResponse::BadRequest().body(format!(
                            "Error decoding basic auth base64: '{}'",
                            auth_headers.to_str().unwrap()
                        ))));
                    }
                };
                if !match_auth(auth_req, required_auth) {
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
