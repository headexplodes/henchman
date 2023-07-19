use lazy_static::lazy_static;
use regex::Regex;
use http::{Request, header::HeaderName};
use base64::Engine;

use crate::server::ServerError;

pub enum AuthorizationValue {
    Basic {
        username: String,
        password: String,
    }
}

fn ellipsis(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}[...]", &s[..max])
    } else {
        s.to_string()
    }
}

pub fn parse_authorization_value(value: &str) -> Result<AuthorizationValue, ServerError> {
    lazy_static! {
        static ref BASIC_PATTERN: Regex = Regex::new("^Basic ([A-Za-z0-9+/=]+)$").unwrap();
    }

    if let Some(captures) = BASIC_PATTERN.captures(value) {
        if let Some(cred_base64) = captures.get(1) {
            // let cred_bytes = base64::decode(cred_base64.as_str()).map_err(|err| {
            //     error!("Error decoding credentials as base64: {}", err);
            //     ServerError::BadRequest(format!("Malformed Authorization header"))
            // })?;
            let cred_bytes = base64::engine::general_purpose::STANDARD_NO_PAD.decode(cred_base64.as_str()).map_err(|err| {
                error!("Error decoding credentials as base64: {}", err);
                ServerError::BadRequest(format!("Malformed Authorization header"))
            })?;

            let cred_str = std::str::from_utf8(&cred_bytes).map_err(|err| {
                error!("Decoded credentials was not a valid UTF-8 string: {}", err);
                ServerError::BadRequest(format!("Malformed Basic credentials"))
            })?;

            let cred_parts: Vec<&str> = cred_str.split(":").collect();

            match &cred_parts[..] {
                &[username, password] => Ok(AuthorizationValue::Basic {
                    username: username.to_owned(),
                    password: password.to_owned(),
                }),
                _ => {
                    error!("Decoded credentials did not contain <username>:<password>");
                    Err(ServerError::BadRequest(format!("Malformed Basic credentials")))
                }
            }
        } else {
            error!("Regular expression missing capture"); // logic error
            Err(ServerError::BadRequest(format!("Malformed Authorization header")))
        }
    } else {
        error!("Unsupported authorization type: {}", ellipsis(value, 8)); // try to avoid actually logging any passwords in full
        Err(ServerError::BadRequest(format!("Unsupported authorization type")))
    }
}

pub fn parse_authorization<T>(req: &Request<T>) -> Result<Option<AuthorizationValue>, ServerError> {
    get_header(req, &http::header::AUTHORIZATION)
        .and_then(|opt| opt.map(|x| parse_authorization_value(&x)).transpose())
}

fn get_header<T>(req: &Request<T>, name: &HeaderName) -> Result<Option<String>, ServerError> {
    req.headers().get(name).map(|value| {
        value.to_str()
            .map(|s| s.to_owned())
            .map_err(|_| ServerError::BadRequest(format!("Error decoding header value: {}", name)))
    }).transpose()
}

// pub fn get_cookie_value<T>(req: &Request<T>, cookie_name: &str) -> Result<Option<String>, ServerError> {
//     get_header(&req, &http::header::COOKIE).map(|result| {
//         result.and_then(|value| {
//             let cookies = parse_cookies(&value);
//             match cookies.get(cookie_name) {
//                 Some(value) => {
//                     Some(value.to_owned())
//                 }
//                 None => None
//             }
//         })
//     })
// }
//
// /// Very rudimentary cookie parsing (can't seem to find a library to do this)
// pub fn parse_cookies(header_value: &str) -> HashMap<String, String> {
//     let mut result: HashMap<String, String> = HashMap::new();
//
//     for cookie in header_value.split("; ") {
//         let parts: Vec<&str> = cookie.split("=").collect();
//
//         // don't explode if we can't parse cookies because we never know what we're going to get from third-party trackers etc.
//         if parts.len() != 2 {
//             warn!("Could not parse cookie (expected 2 parts, was {})", parts.len()); // not logging value because may contain access token
//             continue;
//         }
//
//         result.insert(parts[0].to_owned(), parts[1].to_owned());
//     }
//
//     return result;
// }

