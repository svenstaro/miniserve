use actix_web::http::header;
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};
use sha2::{Digest, Sha256, Sha512};

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

#[derive(Clone, Debug, PartialEq)]
pub enum RequiredAuthPassword {
    Plain(String),
    Sha256(Vec<u8>),
    Sha512(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
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

pub fn compare_hash<T: Digest>(password: String, hash: &Vec<u8>) -> bool {
    get_hash::<T>(password) == *hash
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_hex_eq(expectation: &str, received: Vec<u8>) {
        let bin = hex::decode(expectation).expect("Provided string is not a valid hex code");
        assert_eq!(bin, received);
    }

    #[test]
    fn get_hash_hex_sha256() {
        let expectation = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let received = get_hash::<Sha256>("abc".to_owned());
        assert_hex_eq(expectation, received);
    }

    #[test]
    fn get_hash_hex_sha512() {
        let expectation = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";
        let received = get_hash::<Sha512>("abc".to_owned());
        assert_hex_eq(expectation, received);
    }

    fn create_auth_params(username: &str, password: &str) -> BasicAuthParams {
        BasicAuthParams {
            username: username.to_owned(),
            password: password.to_owned(),
        }
    }

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

    #[test]
    fn match_auth_plain_password_should_pass() {
        assert!(match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hello there", "plain"),
        ));
    }

    #[test]
    fn match_auth_plain_password_should_fail() {
        assert!(!match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hi!", "plain"),
        ));
    }

    #[test]
    fn match_auth_sha256_password_should_pass() {
        assert!(match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hello there", "sha256"),
        ));
    }

    #[test]
    fn match_auth_sha256_password_should_fail() {
        assert!(!match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hi!", "sha256"),
        ));
    }

    #[test]
    fn match_auth_sha512_password_should_pass() {
        assert!(match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hello there", "sha512"),
        ));
    }

    #[test]
    fn match_auth_sha512_password_should_fail() {
        assert!(!match_auth(
            create_auth_params("obi", "hello there"),
            &create_required_auth("obi", "hi!", "sha512"),
        ));
    }
}
