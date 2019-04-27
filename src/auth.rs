use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};
use sha2::{Digest, Sha256, Sha512};

use crate::errors::ContextualError;

pub struct Auth;

#[derive(Clone, Debug)]
/// HTTP Basic authentication parameters
pub struct BasicAuthParams {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq)]
/// `password` field of `RequiredAuth`
pub enum RequiredAuthPassword {
    Plain(String),
    Sha256(Vec<u8>),
    Sha512(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
/// Authentication structure to match `BasicAuthParams` against
pub struct RequiredAuth {
    pub username: String,
    pub password: RequiredAuthPassword,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
pub fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, ContextualError> {
    let basic_removed = authorization_header
        .to_str()
        .map_err(|e| {
            ContextualError::ParseError("HTTP authentication header".to_string(), e.to_string())
        })?
        .replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(ContextualError::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let credentials: Vec<&str> = decoded_str.splitn(2, ':').collect();

    // If argument parsing went fine, it means the HTTP credentials string is well formatted
    // So we can safely unpack the username and the password

    Ok(BasicAuthParams {
        username: credentials[0].to_owned(),
        password: credentials[1].to_owned(),
    })
}

/// Verify authentication
pub fn match_auth(basic_auth: BasicAuthParams, required_auth: &RequiredAuth) -> bool {
    if basic_auth.username != required_auth.username {
        return false;
    }

    match &required_auth.password {
        RequiredAuthPassword::Plain(ref required_password) => {
            basic_auth.password == *required_password
        }
        RequiredAuthPassword::Sha256(password_hash) => {
            compare_hash::<Sha256>(basic_auth.password, password_hash)
        }
        RequiredAuthPassword::Sha512(password_hash) => {
            compare_hash::<Sha512>(basic_auth.password, password_hash)
        }
    }
}

/// Return `true` if hashing of `password` by `T` algorithm equals to `hash`
pub fn compare_hash<T: Digest>(password: String, hash: &[u8]) -> bool {
    get_hash::<T>(password) == hash
}

/// Get hash of a `text`
pub fn get_hash<T: Digest>(text: String) -> Vec<u8> {
    let mut hasher = T::new();
    hasher.input(text);
    hasher.result().to_vec()
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
                        let auth_err = ContextualError::HTTPAuthenticationError(Box::new(err));
                        return Ok(Response::Done(
                            HttpResponse::BadRequest().body(auth_err.to_string()),
                        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest_parametrize;

    /// Return a hashing function corresponds to given name
    fn get_hash_func(name: &str) -> impl FnOnce(String) -> Vec<u8> {
        match name {
            "sha256" => get_hash::<Sha256>,
            "sha512" => get_hash::<Sha512>,
            _ => panic!("Invalid hash method"),
        }
    }

    #[rstest_parametrize(
        password, hash_method, hash,
        case("abc", "sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        case("abc", "sha512", "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"),
    )]
    fn test_get_hash(password: &str, hash_method: &str, hash: &str) {
        let hash_func = get_hash_func(hash_method);
        let expected = hex::decode(hash).expect("Provided hash is not a valid hex code");
        let received = hash_func(password.to_owned());
        assert_eq!(received, expected);
    }

    /// Helper function that creates a `RequiredAuth` structure and encrypt `password` if necessary
    fn create_required_auth(username: &str, password: &str, encrypt: &str) -> RequiredAuth {
        use RequiredAuthPassword::*;

        RequiredAuth {
            username: username.to_owned(),
            password: match encrypt {
                "plain" => Plain(password.to_owned()),
                "sha256" => Sha256(get_hash::<sha2::Sha256>(password.to_owned())),
                "sha512" => Sha512(get_hash::<sha2::Sha512>(password.to_owned())),
                _ => panic!("Unknown encryption type"),
            },
        }
    }

    #[rstest_parametrize(
        should_pass,
        param_username,
        param_password,
        required_username,
        required_password,
        encrypt,
        case(true, "obi", "hello there", "obi", "hello there", "plain"),
        case(false, "obi", "hello there", "obi", "hi!", "plain"),
        case(true, "obi", "hello there", "obi", "hello there", "sha256"),
        case(false, "obi", "hello there", "obi", "hi!", "sha256"),
        case(true, "obi", "hello there", "obi", "hello there", "sha512"),
        case(false, "obi", "hello there", "obi", "hi!", "sha512")
    )]
    fn test_auth(
        should_pass: bool,
        param_username: &str,
        param_password: &str,
        required_username: &str,
        required_password: &str,
        encrypt: &str,
    ) {
        assert_eq!(
            match_auth(
                BasicAuthParams {
                    username: param_username.to_owned(),
                    password: param_password.to_owned(),
                },
                &create_required_auth(required_username, required_password, encrypt),
            ),
            should_pass,
        )
    }
}
