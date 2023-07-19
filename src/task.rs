use std::path::PathBuf;

use either::Either;

pub enum TaskMethod {
    GET,
    POST,
}

pub enum TaskParameterType {
    String,
    Number,
    Boolean,
}

pub enum TaskParameterValue {
    String(String),
    Number(Either<i64, f64>),
    Boolean(bool),
}

pub struct TaskDef {
    pub name: String,
    pub description: Option<String>,
    pub method: Vec<TaskMethod>,
    pub parameters: Vec<TaskDefParameter>,
    pub exec: TaskDefExec,
}

pub struct TaskDefParameter {
    pub name: String,
    pub required: bool,
    pub default: Option<TaskParameterValue>,
    pub _type: TaskParameterType,
    pub _enum: Vec<String>,
    pub env: Option<String>,
}

pub struct TaskDefExec {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub dir: PathBuf,
}

impl TaskDefParameter {
    pub fn validate(&self, str: &str) -> bool {
        match self._type {
            TaskParameterType::String => {
                self._enum.is_empty()
                    || self._enum.iter().any(|x| x == str)
            },
            TaskParameterType::Number => {
                str.parse::<f32>().is_ok()
            }
            TaskParameterType::Boolean => {
                str == "true" || str == "false"
            }
        }
    }
}
