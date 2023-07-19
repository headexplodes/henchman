use std::fs;
use std::io::{Error as IoError};
use std::path::{Path, PathBuf};

use toml::de::Error as TomlError;

use serde::{Deserialize};

use crate::task;
use either::Either;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    GET,
    POST,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub description: Option<String>,
    pub method: Vec<Method>,
    #[serde(default)]
    pub parameters: Vec<TaskParameter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskParameterType {
    String,
    Number,
    Boolean,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum TaskParameterValue {
    String(String),
    Number(serde_json::Number),
    Boolean(bool),
}

#[derive(Debug, Deserialize)]
pub struct TaskParameter {
    pub name: String,
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<TaskParameterValue>,
    #[serde(rename = "type")]
    pub _type: Option<TaskParameterType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "Vec::new")]
    #[serde(rename = "enum")]
    pub _enum: Vec<String>,
    pub env: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Exec {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskFileToml {
    pub task: Task,
    pub exec: Exec,
}

#[derive(Debug)]
pub enum ConfigFileError {
    Toml(TomlError, PathBuf),
    Io(IoError, Option<PathBuf>),
    InvalidTaskFileName(PathBuf),
    InvalidPasswordHash { username: String },
}

const TASK_FILE_SUFFIX: &'static str = ".task.toml";

impl TaskFileToml {
    /// The filename (excluding the '.task.toml' suffix) is the name of the task
    pub fn load(path: &Path) -> Result<task::TaskDef, ConfigFileError> {
        let from_io_err = |err: IoError| -> ConfigFileError {
            ConfigFileError::Io(err, Some(path.into()))
        };

        let from_toml_err = |err: TomlError| -> ConfigFileError {
            ConfigFileError::Toml(err, path.into())
        };

        let file_str = fs::read_to_string(path).map_err(from_io_err)?;

        let model: TaskFileToml = toml::from_str(&file_str).map_err(from_toml_err)?;

        Ok(to_task_def(model, path)?)
    }
}

pub fn find_task_files(dir: &Path) -> Result<Vec<PathBuf>, ConfigFileError> {
    let from_io_err = |err: IoError| -> ConfigFileError {
        ConfigFileError::Io(err, Some(dir.to_owned()))
    };

    let files = fs::read_dir(&dir).map_err(from_io_err)?;

    let mut res = Vec::<PathBuf>::new();

    for entry in files {
        let entry = entry.map_err(from_io_err)?;

        let from_io_err = |err: IoError| -> ConfigFileError {
            ConfigFileError::Io(err, Some(entry.path()))
        };

        let file_type = entry.file_type().map_err(from_io_err)?;

        if file_type.is_dir() {
            res.append(&mut find_task_files(&entry.path())?)
        } else if file_type.is_file() {
            if entry.path().to_string_lossy().ends_with(TASK_FILE_SUFFIX) {
                res.push(entry.path())
            }
        } else if file_type.is_symlink() {
            todo!("Follow symlinks"); // TODO: follow symlinks
        } else {
            panic!("Unexpected file type: {:?}", entry) // not expected
        }
    }

    Ok(res)
}

fn get_task_name(path: &Path) -> Result<String, ConfigFileError> {
    match path.file_name() {
        Some(file_name) => {
            let name = file_name.to_string_lossy().replace(TASK_FILE_SUFFIX, ""); // trip suffix
            if name.trim().is_empty() {
                return Err(ConfigFileError::InvalidTaskFileName(path.to_owned()));
            }
            Ok(name)
        }
        None => {
            Err(ConfigFileError::InvalidTaskFileName(path.to_owned()))
        }
    }
}

fn to_task_method(toml: Method) -> task::TaskMethod {
    match toml {
        Method::GET =>
            task::TaskMethod::GET,
        Method::POST =>
            task::TaskMethod::POST
    }
}

fn to_task_parameter_value(toml: TaskParameterValue) -> task::TaskParameterValue {
    match toml {
        TaskParameterValue::String(s) => {
            task::TaskParameterValue::String(s)
        }
        TaskParameterValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                task::TaskParameterValue::Number(Either::Left(i))
            } else if let Some(f) = n.as_f64() {
                task::TaskParameterValue::Number(Either::Right(f))
            } else {
                panic!("Number is too large") // eg, if can only be stored in a u64 (not currently supported)
            }
        }
        TaskParameterValue::Boolean(b) => {
            task::TaskParameterValue::Boolean(b)
        }
    }
}

fn to_task_parameter_type(toml: TaskParameterType) -> task::TaskParameterType {
    match toml {
        TaskParameterType::String =>
            task::TaskParameterType::String,
        TaskParameterType::Number =>
            task::TaskParameterType::Number,
        TaskParameterType::Boolean =>
            task::TaskParameterType::Boolean
    }
}

fn to_task_def_parameter(toml: TaskParameter) -> task::TaskDefParameter {
    task::TaskDefParameter {
        name: toml.name,
        required: toml.required.unwrap_or(false),
        default: toml.default.map(to_task_parameter_value),
        _type: toml._type.map(to_task_parameter_type)
            .unwrap_or(task::TaskParameterType::String),
        _enum: toml._enum,
        env: toml.env,
    }
}

fn to_task_def_exec(toml: Exec, path: &Path) -> task::TaskDefExec {
    let parent_dir = path.parent().unwrap_or_else(||
        panic!("Path has no parent directory: {:?}", path)); // shouldn't be possible on regular file systems

    let dir = toml.dir
        .map(|d| parent_dir.join(d))
        .unwrap_or_else(|| parent_dir.to_owned());

    task::TaskDefExec {
        command: toml.command,
        args: toml.args,
        dir,
    }
}

fn to_task_def(toml: TaskFileToml, path: &Path) -> Result<task::TaskDef, ConfigFileError> {
    let TaskFileToml { task, exec } = toml;

    let name = get_task_name(path)?;

    let method = if task.method.is_empty() {
        vec![task::TaskMethod::GET,
             task::TaskMethod::POST]
    } else {
        task.method.into_iter().map(to_task_method).collect()
    };

    let parameters = task.parameters.into_iter().map(to_task_def_parameter).collect();

    let exec = to_task_def_exec(exec, path);

    Ok(task::TaskDef {
        name,
        description: task.description,
        method,
        parameters,
        exec,
    })
}