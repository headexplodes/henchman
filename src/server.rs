use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::TryFutureExt;
use futures::stream::StreamExt;

// use tokio_stream::StreamExt;
use tokio_stream::wrappers::LinesStream;

use http::{header, HeaderValue, Method, StatusCode};
use hyper::{Body, Request, Response};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use url::{form_urlencoded, Url};

use crate::{CachedCredential, TaskExec, TaskRequest, UserSession, UserDef};
use crate::json_conv;
use crate::task::{TaskDefParameter, TaskMethod, TaskParameterValue};

#[allow(dead_code)] // they'll be used eventually
#[derive(Debug)]
pub enum ServerError {
    Unauthorized,
    BadRequest(String),
    NotFound,
    MethodNotAllowed,
    Forbidden,
    NotAcceptable,
    InternalServerError,
}

pub struct UserPrincipal {
    pub username: String,
    pub roles: HashSet<String>,
}

impl UserPrincipal {
    pub fn from(user_def: &UserDef) -> UserPrincipal {
        UserPrincipal {
            username: user_def.username.clone(),
            roles: user_def.roles.clone(),
        }
    }
}

fn html_error(status: http::StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(format!("<html><body><h1>{}</h1></body></html>", message)))
        .unwrap()
}

pub async fn handle(shared: Arc<crate::Shared>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let req_method = req.method().to_owned();
    let req_uri = req.uri().to_owned();

    let res = match match_path(shared, req).await {
        Ok(res) => res,
        Err(ServerError::Unauthorized) => {
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("WWW-Authenticate", "Basic realm=Login")
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Body::from(format!("<html><body><h1>{}</h1></body></html>", "Unauthorized")))
                .unwrap()
        }
        Err(ServerError::Forbidden) => {
            html_error(StatusCode::FORBIDDEN, "Forbidden")
        }
        Err(ServerError::BadRequest(message)) => {
            html_error(StatusCode::BAD_REQUEST, &format!("Bad request: {}", message))
        }
        Err(ServerError::NotFound) => {
            html_error(StatusCode::NOT_FOUND, "Not found")
        }
        Err(ServerError::MethodNotAllowed) => {
            html_error(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed")
        }
        Err(ServerError::NotAcceptable) => {
            html_error(StatusCode::NOT_ACCEPTABLE, "Not acceptable")
        }
        Err(ServerError::InternalServerError) => {
            html_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
        }
    };

    info!("Request: {} {} -> {}",
        req_method,
        req_uri,
        res.status());

    Ok(res)
}

fn serve_redirect() -> Result<Response<Body>, ServerError> {
    let response = Response::builder()
        .status(StatusCode::FOUND)
        .header("Location", "/web/tasks")
        .body(Body::empty())
        .unwrap();

    Ok(response)
}

// const SESSION_ID_COOKIE: &'static str = "session_id";

const SESSION_CACHE_PERIOD: Duration = Duration::from_secs(30 * 60); // 30 minutes

fn prune_expired(sessions: &mut HashMap<CachedCredential, UserSession>) -> () {
    let now = Instant::now();

    // let expired: Vec<&CachedCredential> = sessions.iter()
    //     .filter(|(_, v)| v.expires_at.lt(&now))
    //     .map(|(k, _)| k)
    //     .collect();
    //
    // for key in expired {
    //     sessions.remove(key);
    // }

    sessions.retain(|_k, v| {
        v.expires_at.lt(&now)
    });
}

// fn get_user<'a>(shared: &'a Arc<crate::Shared>, username: &str) -> Result<&'a UserDef, ServerError> {
//     let users = shared.users.read().map_err(|err| {
//         error!("Could not obtain users lock: {:?}", err);
//         ServerError::InternalServerError
//     })?;
//
//     users.get(username).ok_or_else(|| {
//         warn!("Username not found: {}", username);
//         ServerError::Unauthorized
//     })
// }

fn ensure_auth(shared: &Arc<crate::Shared>, req: &Request<Body>) -> Result<UserPrincipal, ServerError> {
    let req_auth = crate::utils::parse_authorization(&req)?
        .ok_or(ServerError::Unauthorized)?;

    let has_session = |key: &crate::CachedCredential| -> Result<bool, ServerError> {
        shared.sessions.read()
            .map(|sessions| sessions.contains_key(key))
            .map_err(|err| {
                error!("Could not obtain sessions lock: {:?}", err);
                ServerError::InternalServerError
            })
    };

    match req_auth {
        crate::utils::AuthorizationValue::Basic { username, password } => {
            let users = shared.users.read().map_err(|err| {
                error!("Could not obtain users lock: {:?}", err);
                ServerError::InternalServerError
            })?;

            // let user_def = get_user(shared, &username)?;
            let user_def = users.get(&username).ok_or_else(|| {
                warn!("Username not found: {}", username);
                ServerError::Unauthorized
            })?;

            let cache_hash = crate::CachedCredential {
                username: username.clone(),
                password_hash: ring::digest::digest(&ring::digest::SHA256, password.as_bytes())
                    .as_ref()
                    .to_vec(),
            };

            // found recent session that confirmed password is correct
            if has_session(&cache_hash)? {
                return Ok(UserPrincipal::from(user_def));
            }

            match crate::password::verify_password_parts(&password, &user_def.password) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    warn!("Incorrect password for user: {}", username);
                    Err(ServerError::Unauthorized)
                }
                Err(err) => {
                    error!("Error verifying password: {:?}", err);
                    Err(ServerError::InternalServerError)
                }
            }?;

            // if !crate::password::verify_password_parts(&password, &user_def.password)? {
            //     warn!("Incorrect password for user: {}", username);
            //     return Err(ServerError::Unauthorized);
            // }

            // don't keep this lock while verifying password above (verifying is slow)
            let mut sessions = shared.sessions.write()
                .map_err(|err| {
                    error!("Could not obtain sessions lock: {:?}", err);
                    ServerError::InternalServerError
                })?;

            prune_expired(&mut sessions);

            let expires_at = Instant::now().checked_add(SESSION_CACHE_PERIOD).unwrap_or_else(|| {
                panic!("Instant value overflow"); // not expecting this (our duration value is quite short)
            });

            sessions.insert(cache_hash, UserSession { expires_at });

            Ok(UserPrincipal::from(user_def))
        }
    }
}

async fn match_path(shared: Arc<crate::Shared>, req: Request<Body>) -> Result<Response<Body>, ServerError> {
    let uri = req.uri().clone();

    let path: Vec<&str> = uri.path().split("/").skip(1).collect();

    ensure_auth(&shared, &req)?;

    let res = match &path[..] {
        [""] => {
            serve_redirect()
        }
        ["api", tail @ ..] => {
            match_path_api(shared, req, tail).await
        }
        all @ ["favicon.ico"] => {
            crate::web::serve_static(all)
        }
        ["web", tail @ ..] => {
            crate::web::match_path_web(tail).await
        }
        _ => Err(ServerError::NotFound)
    };

    if let Err(ref err) = res {
        info!("Error: {:?}", err);
    };

    res
}

async fn match_path_api(shared: Arc<crate::Shared>, req: Request<Body>, path: &[&str]) -> Result<Response<Body>, ServerError> {
    match &path[..] {
        ["tasks"] => {
            handle_tasks(shared, req)
        }
        ["tasks", task_name] => {
            handle_task(shared, req, task_name.to_owned()).await
        }
        ["tasks", task_name, "run"] => {
            handle_task_run(shared, req, task_name.to_owned()).await
        }
        _ => {
            Err(ServerError::NotFound)
        }
    }
}

fn handle_tasks(shared: Arc<crate::Shared>, req: Request<Body>) -> Result<Response<Body>, ServerError> {
    if req.method() != Method::GET {
        return Err(ServerError::MethodNotAllowed);
    }

    let tasks = shared.tasks.read().unwrap(); // TODO: handle error

    let tasks_json: Vec<crate::json::TaskJson> = tasks.iter()
        .map(|(_, task)| json_conv::to_task_json(task))
        .collect();

    let tasks_bytes = serde_json::to_vec(&tasks_json).unwrap(); // TODO: handle error

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(Body::from(tasks_bytes))
        .unwrap(); // TODO: handle error

    Ok(response)
}

async fn handle_task(shared: Arc<crate::Shared>, req: Request<Body>, task_name: &str) -> Result<Response<Body>, ServerError> {
    if req.method() != Method::GET {
        return Err(ServerError::MethodNotAllowed);
    }

    let tasks = shared.tasks.read().unwrap(); // TODO: handle error

    let task_json = tasks.get(task_name)
        .map(|task| json_conv::to_task_json(task));

    let task_json = match task_json {
        Some(x) => x,
        None => {
            return Err(ServerError::NotFound);
        }
    };

    let tasks_bytes = serde_json::to_vec(&task_json).unwrap(); // TODO: handle error

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(Body::from(tasks_bytes))
        .unwrap(); // TODO: handle error

    Ok(response)
}

async fn handle_task_run(shared: Arc<crate::Shared>, req: Request<Body>, task_name: &str) -> Result<Response<Body>, ServerError> {
    let task_req = parse_task_req(req, task_name).await?;

    let task_exec = validate_task_req(shared, task_req)?;

    info!("Executing task: {}", task_name);

    exec_task(task_exec)
}

async fn parse_task_req(req: Request<Body>, task_name: &str) -> Result<TaskRequest, ServerError> {
    let method = req.method().clone();

    let params = match *req.method() {
        Method::POST => {
            match req.headers().get(header::CONTENT_TYPE) {
                Some(value) => {
                    if value == HeaderValue::from_static("application/x-www-form-urlencoded") {
                        let body = hyper::body::to_bytes(req.into_body()).await.map_err(|err| {
                            error!("Error reading request body: {}", err);
                            ServerError::InternalServerError
                        })?;

                        form_urlencoded::parse(&body)
                            .map(|(a, b)| (a.to_string(), b.to_string()))
                            .collect::<HashMap<String, String>>()
                    } else {
                        error!("Unexpected content type: {:?}", value);
                        return Err(ServerError::NotAcceptable);
                    }
                }
                None => {
                    HashMap::new() // no parameters (OK, as long as none are required)
                }
            }
        }
        Method::GET => {
            let url = format!("http://127.0.0.1/{}", req.uri().to_string()); // URLs must be relative
            Url::parse(&url)
                .expect(&format!("Malformed URL: {}", url))
                .query_pairs()
                .into_owned()
                .collect()
        }
        _ => {
            return Err(ServerError::MethodNotAllowed);
        }
    };

    let task_req = TaskRequest {
        name: task_name.to_owned(),
        method,
        params,
    };

    Ok(task_req)
}

fn param_to_string(param: &TaskParameterValue) -> String {
    match param {
        TaskParameterValue::String(s) => s.clone(),
        TaskParameterValue::Number(n) => n.to_string(),
        TaskParameterValue::Boolean(b) => b.to_string()
    }
}

fn validate_params(req_params: HashMap<String, String>,
                   task_params: &Vec<TaskDefParameter>) -> Result<HashMap<String, String>, ServerError> {
    let mut result = HashMap::<String, String>::new();

    for task_param in task_params {
        let req_value = req_params.get(&task_param.name);

        let value = if req_value.is_some() {
            let req_value = req_value.unwrap().clone();

            if !task_param.validate(&req_value) {
                return Err(ServerError::BadRequest(format!("Invalid parameter value: {}", task_param.name)));
            }

            Some(req_value)
        } else if task_param.default.is_some() {
            task_param.default.as_ref().map(param_to_string)
        } else if !task_param.required {
            None
        } else {
            return Err(ServerError::BadRequest(format!("Parameter is required: {}", task_param.name)));
        };

        if let Some(value) = value {
            result.insert(task_param.name.clone(), value.to_owned());
        }
    }

    let task_names: HashSet<&str> = task_params.iter().map(|t| t.name.as_ref()).collect();
    let req_names: HashSet<&str> = req_params.keys().map(|t| t.as_ref()).collect();

    let unknown: HashSet<&&str> = req_names.difference(&task_names).collect();
    if !unknown.is_empty() {
        warn!("Unexpected task parameter(s): {:?}", unknown);
    }

    Ok(result)
}

fn validate_task_req(shared: Arc<crate::Shared>, task_req: TaskRequest) -> Result<TaskExec, ServerError> {
    let tasks = shared.tasks.read().unwrap();

    let task_def = match tasks.get(&task_req.name) {
        Some(task) => task,
        None => {
            error!("Task not found: {}", task_req.name);
            return Err(ServerError::NotFound);
        }
    };

    let allowed_methods: HashSet<Method> = task_def.method.iter()
        .map(|method| match method {
            TaskMethod::GET => Method::GET,
            TaskMethod::POST => Method::POST
        }).collect();

    if !allowed_methods.contains(&task_req.method) {
        error!("Method not allowed for task: {}", task_req.method);
        return Err(ServerError::MethodNotAllowed);
    }

    let params = validate_params(task_req.params, &task_def.parameters)?;

    let env: HashMap<String, String> = task_def.parameters.iter()
        .flat_map(|task_param| {
            match (&task_param.env, params.get(&task_param.name)) {
                (Some(env), Some(value)) => Some((env.to_owned(), value.to_owned())),
                _ => None
            }
        })
        .collect();

    let args = task_def.exec.args.as_ref().map(|x| x.clone()).unwrap_or(Vec::new());

    Ok(TaskExec {
        command: task_def.exec.command.clone(),
        args,
        dir: task_def.exec.dir.clone(),
        env,
    })
}

fn exec_task(task: TaskExec) -> Result<Response<Body>, ServerError> {
    info!("Executing: {:?}", task); // TODO: may need a 'secret' parameter type to avoid logging secret parameters here

    let args: Vec<OsString> = task.args
        .iter()
        .map(|a| OsString::from(&a))
        .collect();

    let mut command = Command::new(&task.command);

    // info!("Running command: program = \"{}\", dir = \"{:?}\", args = \"{:?}\"", task.command, task.dir, args);

    // kill process if the connection is dropped (if nobody is around to see output process shouldn't keep running)
    command.current_dir(task.dir)
        .args(&args)
        .envs(&task.env)
        .kill_on_drop(true) // this seems to leave zombies around...
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|e| {
        error!("Error executing command: {:?}", e);
        ServerError::InternalServerError
    })?;

    let stdout = child.stdout.take().unwrap(); // TODO: handle not having both streams
    let stderr = child.stderr.take().unwrap();

    // not buffering so that all output (however small) is sent through to browser
    let stdout_stream = LinesStream::new(BufReader::with_capacity(1, stdout).lines()).map(|line| line.map(|l| l + "\n"));
    let stderr_stream = LinesStream::new(BufReader::with_capacity(1, stderr).lines()).map(|line| line.map(|l| l + "\n"));

    let interleaved = crate::interleave::Interleave::new(
        stdout_stream.fuse(),
        stderr_stream.fuse());

    // using 'wait_with_output()' rather than 'wait()' (even though we don't need the process output), as this function takes ownership of child (not sure if there's a better way to do this)
    let exit_code_fut = child.wait_with_output().map_ok(|output| {
        let exit_code = output.status;

        info!("Process exited with code: {}",
            exit_code.code()
            .map(|x| x.to_string())
            .unwrap_or("<none>".to_owned()));

        format!("[Exit code: {}]",
                exit_code.code()
                    .map(|x| x.to_string())
                    .unwrap_or("<unknown>".to_owned()))
    });

    let exit_code = futures::stream::once(exit_code_fut);
    // let exit_code = futures::stream::empty();;

    let chained = interleaved.chain(exit_code);

    let body = Body::wrap_stream(chained);

    // being very explicit about content type to prevent browser from buffering (so every line is printed as it executes)
    let response = Response::builder()
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap();

    Ok(response)
}