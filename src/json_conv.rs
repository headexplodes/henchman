use crate::json::*;
use crate::task::{TaskDef, TaskMethod, TaskParameterType, TaskParameterValue, TaskDefParameter};

impl From<&TaskMethod> for MethodJson {
    fn from(model: &TaskMethod) -> Self {
        match model {
            TaskMethod::GET =>
                MethodJson::GET,
            TaskMethod::POST =>
                MethodJson::POST
        }
    }
}

impl From<&TaskParameterType> for TaskParameterTypeJson {
    fn from(model: &TaskParameterType) -> Self {
        match model {
            TaskParameterType::String =>
                TaskParameterTypeJson::String,
            TaskParameterType::Number =>
                TaskParameterTypeJson::Number,
            TaskParameterType::Boolean =>
                TaskParameterTypeJson::Boolean,
        }
    }
}

impl From<&TaskParameterValue> for TaskParameterValueJson {
    fn from(model: &TaskParameterValue) -> Self {
        match model {
            TaskParameterValue::String(s) =>
                TaskParameterValueJson::String(s.clone()),
            TaskParameterValue::Number(n) =>
                TaskParameterValueJson::Number(n.either(
                    |i| i.into(),
                    |f| serde_json::Number::from_f64(f).expect("Expected finite number"))),
            TaskParameterValue::Boolean(b) =>
                TaskParameterValueJson::Boolean(b.clone())
        }
    }
}

impl From<&TaskDefParameter> for TaskParameterJson {
    fn from(model: &TaskDefParameter) -> Self {
        TaskParameterJson {
            name: model.name.clone(),
            required: model.required,
            default: model.default.as_ref().map(|x| x.into()),
            _type: (&model._type).into(),
            _enum: model._enum.iter().map(|x| x.into()).collect(),
        }
    }
}

pub fn to_task_json(model: &TaskDef) -> TaskJson {
    TaskJson {
        name: model.name.clone(),
        description: model.description.clone(),
        method: model.method.iter().map(Into::into).collect(),
        parameters: model.parameters.iter().map(Into::into).collect(),
    }
}
