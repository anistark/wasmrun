//! Agent mode: LLM tool schema definitions.
//!
//! Provides tool definitions in OpenAI and Anthropic function-calling formats
//! so LLM agents can discover and invoke sandbox operations.

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct OpenAiTool {
    pub r#type: &'static str,
    pub function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
pub struct OpenAiFunction {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Serialize)]
pub struct AnthropicTool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn openai_tools() -> Vec<OpenAiTool> {
    vec![
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "create_session",
                description: "Create a new isolated WASM sandbox session with its own filesystem and environment.",
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }),
            },
        },
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "execute_code",
                description: "Execute a WASM file inside a sandbox session. Returns stdout, stderr, exit code, and duration.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "The session ID returned by create_session"
                        },
                        "wasm_path": {
                            "type": "string",
                            "description": "Path to the .wasm file relative to the session root"
                        },
                        "function": {
                            "type": "string",
                            "description": "Exported function to call (defaults to _start or main)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments passed to the WASM program"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Execution timeout in seconds (default: 30)"
                        },
                        "env": {
                            "type": "object",
                            "additionalProperties": { "type": "string" },
                            "description": "Environment variables to set before execution"
                        }
                    },
                    "required": ["session_id", "wasm_path"],
                    "additionalProperties": false
                }),
            },
        },
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "write_file",
                description: "Write a file to the session's isolated filesystem. Parent directories are created automatically.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "The session ID"
                        },
                        "path": {
                            "type": "string",
                            "description": "File path relative to session root"
                        },
                        "content": {
                            "type": "string",
                            "description": "File content to write"
                        }
                    },
                    "required": ["session_id", "path", "content"],
                    "additionalProperties": false
                }),
            },
        },
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "read_file",
                description: "Read a file from the session's filesystem.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "The session ID"
                        },
                        "path": {
                            "type": "string",
                            "description": "File path relative to session root"
                        }
                    },
                    "required": ["session_id", "path"],
                    "additionalProperties": false
                }),
            },
        },
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "list_files",
                description: "List files and directories at a path in the session's filesystem.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "The session ID"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory path relative to session root (default: /)"
                        }
                    },
                    "required": ["session_id"],
                    "additionalProperties": false
                }),
            },
        },
        OpenAiTool {
            r#type: "function",
            function: OpenAiFunction {
                name: "destroy_session",
                description: "Destroy a sandbox session and clean up all its files and resources.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "The session ID to destroy"
                        }
                    },
                    "required": ["session_id"],
                    "additionalProperties": false
                }),
            },
        },
    ]
}

pub fn anthropic_tools() -> Vec<AnthropicTool> {
    openai_tools()
        .into_iter()
        .map(|t| AnthropicTool {
            name: t.function.name,
            description: t.function.description,
            input_schema: t.function.parameters,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_tools_valid_structure() {
        let tools = openai_tools();
        assert_eq!(tools.len(), 6);
        for tool in &tools {
            assert_eq!(tool.r#type, "function");
            assert!(!tool.function.name.is_empty());
            assert!(!tool.function.description.is_empty());
            assert!(tool.function.parameters.is_object());
            assert_eq!(tool.function.parameters["type"], "object");
            assert!(tool.function.parameters["properties"].is_object());
            assert!(tool.function.parameters["required"].is_array());
        }
    }

    #[test]
    fn test_openai_tool_names() {
        let tools = openai_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.function.name).collect();
        assert!(names.contains(&"create_session"));
        assert!(names.contains(&"execute_code"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"destroy_session"));
    }

    #[test]
    fn test_execute_code_has_required_params() {
        let tools = openai_tools();
        let exec = tools
            .iter()
            .find(|t| t.function.name == "execute_code")
            .unwrap();
        let required = exec.function.parameters["required"].as_array().unwrap();
        let req_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(req_strs.contains(&"session_id"));
        assert!(req_strs.contains(&"wasm_path"));
    }

    #[test]
    fn test_anthropic_tools_same_count() {
        assert_eq!(openai_tools().len(), anthropic_tools().len());
    }

    #[test]
    fn test_anthropic_tools_valid_structure() {
        let tools = anthropic_tools();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn test_anthropic_tool_names_match_openai() {
        let openai_names: Vec<&str> = openai_tools().iter().map(|t| t.function.name).collect();
        let anthropic_names: Vec<&str> = anthropic_tools().iter().map(|t| t.name).collect();
        assert_eq!(openai_names, anthropic_names);
    }

    #[test]
    fn test_openai_tools_serializable() {
        let tools = openai_tools();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("create_session"));
        assert!(json.contains("\"type\":\"function\""));
    }

    #[test]
    fn test_anthropic_tools_serializable() {
        let tools = anthropic_tools();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("input_schema"));
        assert!(json.contains("destroy_session"));
    }

    #[test]
    fn test_tools_roundtrip_parse() {
        let tools = openai_tools();
        let json = serde_json::to_string(&tools).unwrap();
        let parsed: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 6);
        assert_eq!(parsed[0]["type"], "function");
    }
}
