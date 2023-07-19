use std::collections::HashMap;

use http::{Response, StatusCode};
use hyper::body::Body;

use lazy_static::lazy_static;

use crate::server::ServerError;

pub async fn match_path_web(path: &[&str]) -> Result<Response<Body>, ServerError> {
    match &path[..] {
        ["tasks", _name] => {
            serve_static(&["tasks", "task"])
        }
        [tail @ ..] => {
            serve_static(tail)
        }
    }
}

struct WebResource {
    req_path: &'static [&'static str],
    #[cfg(debug_load_files)]
    file_path: &'static str,
    bytes: &'static [u8],
    content_type: &'static str,
}

const TEXT_HTML: &'static str = "text/html; charset=utf-8";
const TEXT_CSS: &'static str = "text/css";
const APPLICATION_JAVASCRIPT: &'static str = "application/javascript";
const IMAGE_PNG: &'static str = "image/png";

macro_rules! resource {
    ($req_path: expr, $file_path: literal, $content_type: expr) => {
        WebResource {
            req_path: $req_path,
            #[cfg(debug_load_files)]
            file_path: $file_path,
            bytes: include_bytes!($file_path),
            content_type: $content_type,
        }
    };
}

const WEB_RESOURCES: &'static [WebResource] = &[
    resource!(&["tasks"], "resources/tasks.html", TEXT_HTML),
    resource!(&["tasks", "task"], "resources/tasks/task.html", TEXT_HTML),
    resource!(&["favicon.ico"], "resources/favicon.ico", IMAGE_PNG),
    resource!(&["main.css"], "resources/main.css", TEXT_CSS),
    resource!(&["modules", "api"], "resources/modules/api.mjs", APPLICATION_JAVASCRIPT),
    resource!(&["modules", "html"], "resources/modules/html.mjs", APPLICATION_JAVASCRIPT),
    resource!(&["modules", "task"], "resources/modules/task.mjs", APPLICATION_JAVASCRIPT),
    resource!(&["modules", "tasks"], "resources/modules/tasks.mjs", APPLICATION_JAVASCRIPT),
    resource!(&["modules", "utils"], "resources/modules/utils.mjs", APPLICATION_JAVASCRIPT)
];

lazy_static! {
    static ref WEB_RESOURCES_MAP: HashMap<&'static [&'static str], &'static WebResource> = WEB_RESOURCES.iter().map(|x| (x.req_path, x)).collect();
}

#[cfg(not(debug_load_files))]
fn get_resource(resource: &WebResource) -> Body {
    Body::from(resource.bytes)
}

/// During development it can be handy to load files from disk every time,
/// in release build we don't want to have to ship with extra files (ie, they're all embedded).
#[cfg(debug_load_files)]
fn get_resource(resource: &WebResource) -> Body {
    fn load_file(path: &str) -> Vec<u8> {
        use std::fs::File;
        use std::io::prelude::*;

        let mut file = File::open(path).expect(&format!("Error opening file: {:?}", path.clone()));

        let mut result: Vec<u8> = Vec::new();
        file.read_to_end(&mut result).expect("Error reading file");
        result
    }

    Body::from(load_file(&format!("src/{}", resource.file_path)))
}

pub fn serve_static(path: &[&str]) -> Result<Response<Body>, ServerError> {
    if let Some(resource) = WEB_RESOURCES_MAP.get(path) {
        let body = get_resource(&resource);

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", resource.content_type)
            .header("Cache-Control", "no-cache")
            .body(body)
            .unwrap();

        Ok(response)
    } else {
        Err(ServerError::NotFound)
    }
}