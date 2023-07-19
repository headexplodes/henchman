///
/// API JSON interface (as opposed to configuration file JSON format)
///

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum MethodJson {
    GET,
    POST,
}

#[derive(Serialize, Deserialize)]
pub struct TaskJson {
    pub name: String,
    pub description: Option<String>,
    pub method: Vec<MethodJson>,
    pub parameters: Vec<TaskParameterJson>,
}

#[derive(Serialize, Deserialize)]
pub struct TaskParameterJson {
    pub name: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<TaskParameterValueJson>,
    #[serde(rename = "type")]
    pub _type: TaskParameterTypeJson,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "enum")]
    pub _enum: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum TaskParameterValueJson {
    String(String),
    Number(serde_json::Number),
    Boolean(bool),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskParameterTypeJson {
    String,
    Number,
    Boolean,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct ExampleJson {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub default: Option<TaskParameterValueJson>
    }

    #[test]
    fn test_serialise_default_string() {
        let json = ExampleJson {
            default: Some(TaskParameterValueJson::String("foo".to_owned()))
        };
        assert_eq!(serde_json::to_string(&json).unwrap(), "{\"default\":\"foo\"}");
    }

    #[test]
    fn test_serialise_default_number() {
        let json = ExampleJson {
            default: Some(TaskParameterValueJson::Number(1234.into()))
        };
        assert_eq!(serde_json::to_string(&json).unwrap(), "{\"default\":1234}");
    }

    #[test]
    fn test_serialise_default_boolean() {
        let json = ExampleJson {
            default: Some(TaskParameterValueJson::Boolean(true))
        };
        assert_eq!(serde_json::to_string(&json).unwrap(), "{\"default\":true}");
    }

    #[test]
    fn test_serialise_default_none() {
        let json = ExampleJson {
            default: None
        };
        assert_eq!(serde_json::to_string(&json).unwrap(), "{}");
    }

    #[test]
    fn test_deserialise_default_string() {
        let json: ExampleJson = serde_json::from_str("{\"default\":\"foo\"}").unwrap();
        assert_eq!(json.default, Some(TaskParameterValueJson::String("foo".to_owned())));
    }

    #[test]
    fn test_deserialise_default_number() {
        let json: ExampleJson = serde_json::from_str("{\"default\":1234}").unwrap();
        assert_eq!(json.default, Some(TaskParameterValueJson::Number(1234.into())));
    }

    #[test]
    fn test_deserialise_default_boolean() {
        let json: ExampleJson = serde_json::from_str("{\"default\":true}").unwrap();
        assert_eq!(json.default, Some(TaskParameterValueJson::Boolean(true)));
    }

    #[test]
    fn test_deserialise_default_none() {
        let json: ExampleJson = serde_json::from_str("{}").unwrap();
        assert_eq!(json.default, None);
    }
}