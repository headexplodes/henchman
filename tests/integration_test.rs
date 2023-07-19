extern crate henchman;

#[macro_use]
extern crate log;
extern crate tokio;
extern crate pretty_assertions;
extern crate url;

use pretty_assertions::{assert_eq};

use henchman::{ServerConfig, run_server};

use tokio::sync::oneshot;

use std::env;
use std::net::SocketAddr;
use std::sync::Once;

use futures::{Future, TryFutureExt, FutureExt};

use log::LevelFilter;

use http::{Request, Response, Method, StatusCode};
use http::header::{self, HeaderValue};

use hyper::{Body, Client};

use serde_json::{Value, json};

use url::form_urlencoded;

// const LISTEN_ADDR: &'static str = "127.0.0.1:0"; // choose a free port for each test

const CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

//noinspection SpellCheckingInspection
const DEFAULT_BASIC_AUTH: &'static str = "Basic YWRtaW46c2VjcmV0"; // base-64 encoded 'admin:secret'

use std::error::{Error as StdError};
use std::fmt::{Display, Formatter, Result as FormatResult};
use http::Uri;
use std::collections::HashSet;
use std::path::PathBuf;
use regex::Regex;

#[derive(Debug)]
struct RuntimeError(String);

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FormatResult {
        write!(f, "RuntimeError({})", self.0)
    }
}

impl StdError for RuntimeError {}

type GenericError = Box<dyn StdError>;

async fn start_server() -> (SocketAddr, impl Future<Output=Result<(), Box<dyn StdError + Send>>>, oneshot::Sender<()>) {
    let (started_tx, started_rx) = oneshot::channel::<SocketAddr>();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let resources_dir: PathBuf = format!("{}/tests/resources", CARGO_MANIFEST_DIR).into();

    let config = ServerConfig {
        config: resources_dir.join("server.toml")
    };

    let server_fut = tokio::spawn(async move {
        run_server(config, move |local_addr| {
            started_tx.send(local_addr).expect("Started receiver was dropped");
        }, shutdown_rx.unwrap_or_else(|_| panic!("Shutdown sender was dropped"))).await
            .map_err(|e| {
                error!("Error starting server: {}", e); // otherwise not logged anywhere
                e
            })
    });

    let server_fut = server_fut
        .map(|res| -> Result<(), Box<dyn StdError + Send>> {
            match res {
                Ok(x) => x,
                Err(err) => Err(Box::new(RuntimeError(format!("Error joining server task: {:?}", err))))
            }
        });

    info!("Waiting for server to start...");

    let local_addr = started_rx.await.expect("Sender was dropped");

    info!("Server started on http://{}", local_addr);

    (local_addr, server_fut, shutdown_tx)
}

static LOG_INIT: Once = Once::new();

async fn init_test() -> Result<(SocketAddr, impl Future<Output=Result<(), GenericError>>), GenericError> {
    LOG_INIT.call_once(|| {
        env_logger::builder().filter_level(LevelFilter::Info).try_init().unwrap();
    });

    let (local_addr, server_fut, shutdown_tx) = start_server().await;

    let server_fut = async {
        shutdown_tx.send(()).expect("Shutdown receiver was dropped");

        match server_fut.await {
            Ok(()) => {
                info!("Test server ended normally");
                Ok(())
            }
            Err(ended_err) => {
                error!("Test server ended with error: {:?}", ended_err);
                Err(Box::new(RuntimeError(format!("Unclean server shutdown"))) as Box<dyn StdError>)
            }
        }
    };

    Ok((local_addr, server_fut))
}

fn expected_task_json() -> serde_json::Value {
    json!({
        "name": "example1",
        "description": "Example 1",
        "method": [
            "POST"
        ],
        "parameters": [
            {
                "name": "param1",
                "required": true,
                "type": "string",
                "enum": [
                    "foo",
                    "bar"
                ]
            },
            {
                "name": "param2",
                "required": false,
                "type": "number",
                "default": 3
            }
        ]
    })
}

#[tokio::test]
async fn test_get_tasks() -> Result<(), GenericError> {
    let (local_addr, server_fut) = init_test().await?;

    let client = Client::new();

    let uri: Uri = format!("http://{}/api/tasks", local_addr).parse()?;

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(Body::empty())
        .unwrap();

    let res: Response<hyper::Body> = client.request(req).await?;

    assert_eq!(res.status(), StatusCode::OK);

    let res_bytes = hyper::body::to_bytes(res.into_body()).await?;

    let res_json: Value = serde_json::from_slice(&res_bytes)?;

    let res_names: HashSet<&str> = res_json.as_array().unwrap()
        .iter()
        .map(|x| x.as_object()
            .unwrap()
            .get("name").unwrap()
            .as_str().unwrap())
        .collect();

    let expected_names: HashSet<&str> = vec![
        "example1",
        "param_boolean",
        "param_enum",
        "param_number",
        "param_required",
    ].into_iter().collect();

    assert_eq!(res_names, expected_names);

    // let expected_json = json!([expected_task_json()]);

    // assert_eq!(res_json, expected_json);

    server_fut.await
}

#[tokio::test]
async fn test_get_task() -> Result<(), GenericError> {
    let (local_addr, server_fut) = init_test().await?;

    let client = Client::new();

    let uri: Uri = format!("http://{}/api/tasks/{}", local_addr, "example1").parse()?;

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(Body::empty())
        .unwrap();

    let res: Response<hyper::Body> = client.request(req).await?;

    assert_eq!(res.status(), StatusCode::OK);

    let res_bytes = hyper::body::to_bytes(res.into_body()).await?;

    let res_json: Value = serde_json::from_slice(&res_bytes)?;

    let expected_json = expected_task_json();

    assert_eq!(res_json, expected_json);

    server_fut.await
}

#[tokio::test]
async fn test_post_task_run() -> Result<(), Box<dyn std::error::Error>> {
    let (local_addr, server_fut) = init_test().await?;

    let client = Client::new();

    let uri: Uri = format!("http://{}/api/tasks/example1/run", local_addr).parse()?;

    let req_form: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("param1", "foo")
        .finish();

    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("application/x-www-form-urlencoded"))
        .body(hyper::Body::from(req_form))?;

    let res: Response<hyper::Body> = client.request(req).await?;

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get(header::CONTENT_TYPE), Some(&HeaderValue::from_static("text/plain; charset=utf-8")));

    let res_bytes = hyper::body::to_bytes(res.into_body()).await?;

    let res_text = std::str::from_utf8(&res_bytes)?;

    let expected_text = "Parameter 1: foo\nParameter 2: 3\n[Exit code: 0]"; // expect default for second parameter

    assert_eq!(res_text, expected_text);

    server_fut.await
}

async fn get_response_text(res: http::Response<hyper::Body>) -> String {
    assert_eq!(res.headers().get(header::CONTENT_TYPE), Some(&HeaderValue::from_static("text/plain; charset=utf-8")));

    let res_bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();

    std::str::from_utf8(&res_bytes).map(|x| x.to_owned()).unwrap()
}

async fn get_response_html(res: http::Response<hyper::Body>) -> String {
    assert_eq!(res.headers().get(header::CONTENT_TYPE), Some(&HeaderValue::from_static("text/html; charset=utf-8")));

    let res_bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();

    let res_str = std::str::from_utf8(&res_bytes).map(|x| x.to_owned()).unwrap();

    // find the error message in the response (yeah I know, don't parse HTML with regular expressions blah blah...)
    Regex::new("<h1>(.+)</h1>")
        .unwrap()
        .captures(&res_str)
        .expect("Expected response to contain <h1> tag")
        .get(1)
        .expect("Expected pattern to match")
        .as_str()
        .to_owned()
}

async fn _should_validate_parameter_type(task_name: &str,
                                         bad_value: &str,
                                         good_value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (local_addr, server_fut) = init_test().await?;

    let client = Client::new();

    // Given task with a typed parameter
    let uri: Uri = format!("http://{}/api/tasks/{}/run?param1={}", local_addr, task_name, bad_value).parse()?;

    // When I attempt to execute the task with an invalid value
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(hyper::Body::empty())?;

    let res: Response<hyper::Body> = client.request(req).await?;

    // Then the request should fail
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(get_response_html(res).await, "Bad request: Invalid parameter value: param1");

    // When I execute the task with a valid value
    let uri: Uri = format!("http://{}/api/tasks/{}/run?param1={}", local_addr, task_name, good_value).parse()?;

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(hyper::Body::empty())?;

    let res: Response<hyper::Body> = client.request(req).await?;

    // Then the request should succeed
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(get_response_text(res).await, format!("Success: {}\n[Exit code: 0]", good_value));

    server_fut.await
}

#[tokio::test]
async fn should_validate_parameter_type_boolean() -> Result<(), Box<dyn std::error::Error>> {
    _should_validate_parameter_type("param_boolean", "foo", "true").await
}

#[tokio::test]
async fn should_validate_parameter_type_number() -> Result<(), Box<dyn std::error::Error>> {
    _should_validate_parameter_type("param_number", "foo", "3.14").await
}

#[tokio::test]
async fn should_validate_parameter_type_enum() -> Result<(), Box<dyn std::error::Error>> {
    _should_validate_parameter_type("param_enum", "foo", "bar").await
}

#[tokio::test]
async fn should_validate_parameter_required() -> Result<(), Box<dyn std::error::Error>> {
    let (local_addr, server_fut) = init_test().await?;

    let client = Client::new();

    // Given task with a required parameter
    let uri: Uri = format!("http://{}/api/tasks/param_required/run", local_addr).parse()?;

    // When I attempt to execute the task without providing a value
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(hyper::Body::empty())?;

    let res: Response<hyper::Body> = client.request(req).await?;

    // Then the request should fail
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(get_response_html(res).await, "Bad request: Parameter is required: param1");

    // When I execute the task with a value
    let uri: Uri = format!("http://{}/api/tasks/param_required/run?param1=foo", local_addr).parse()?;

    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, DEFAULT_BASIC_AUTH)
        .body(hyper::Body::empty())?;

    let res: Response<hyper::Body> = client.request(req).await?;

    // Then the request should succeed
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(get_response_text(res).await, format!("Success: foo\n[Exit code: 0]"));

    server_fut.await
}


