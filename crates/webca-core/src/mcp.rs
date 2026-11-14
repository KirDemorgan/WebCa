use serde_json::{Value, json};

use crate::{CameraDevice, CameraProfiles, ControlId, ControlSetting, apply, cli, snapshot};

const LATEST_PROTOCOL: &str = "2025-06-18";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// A Model Context Protocol server exposing one camera and its saved profiles as tools.
#[derive(Debug)]
pub struct McpServer<'a> {
    camera: &'a dyn CameraDevice,
    profiles: Option<CameraProfiles>,
}

impl<'a> McpServer<'a> {
    #[must_use]
    pub fn new(camera: &'a dyn CameraDevice, profiles: Option<CameraProfiles>) -> Self {
        Self { camera, profiles }
    }

    /// Handles one JSON-RPC message; returns the response line, or `None` for notifications.
    pub fn handle(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => return Some(failure(&Value::Null, PARSE_ERROR, &error.to_string())),
        };
        let id = message.get("id").cloned()?;
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Some(failure(&id, INVALID_REQUEST, "missing method"));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => success(&id, &initialize(&params)),
            "ping" => success(&id, &json!({})),
            "tools/list" => success(&id, &json!({ "tools": tools() })),
            "tools/call" => match self.call(&params) {
                Ok(result) => success(&id, &result),
                Err(message) => failure(&id, INVALID_PARAMS, &message),
            },
            _ => failure(&id, METHOD_NOT_FOUND, &format!("unknown method `{method}`")),
        })
    }

    fn call(&mut self, params: &Value) -> Result<Value, String> {
        let name = params.get("name").and_then(Value::as_str).ok_or("missing tool name")?;
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let outcome = match name {
            "list_controls" => self.list_controls(),
            "get_control" => self.get_control(&arguments),
            "set_control" => self.set_control(&arguments),
            "reset_control" => self.reset_control(&arguments),
            "list_profiles" => Ok(self.profiles.as_ref().map_or_else(String::new, cli::format_profiles)),
            "apply_profile" => self.apply_profile(&arguments),
            other => return Err(format!("unknown tool `{other}`")),
        };
        Ok(match outcome {
            Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
            Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
        })
    }

    fn list_controls(&self) -> Result<String, String> {
        snapshot(self.camera).map(|states| cli::format_controls(&states)).map_err(|error| error.to_string())
    }

    fn get_control(&self, arguments: &Value) -> Result<String, String> {
        let id = control(arguments)?;
        let setting = self.camera.read(id).map_err(|error| error.to_string())?;
        Ok(describe(id, setting))
    }

    fn set_control(&self, arguments: &Value) -> Result<String, String> {
        let id = control(arguments)?;
        let setting = match arguments.get("value") {
            Some(Value::String(text)) if text.eq_ignore_ascii_case("auto") => {
                ControlSetting::auto(self.camera.read(id).map_err(|error| error.to_string())?.value)
            }
            Some(value) => ControlSetting::manual(
                value
                    .as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or("`value` must be an integer or \"auto\"")?,
            ),
            None => return Err("`value` is required".into()),
        };
        self.camera.write(id, setting).map_err(|error| error.to_string())?;
        let applied = self.camera.read(id).map_err(|error| error.to_string())?;
        Ok(describe(id, applied))
    }

    fn reset_control(&self, arguments: &Value) -> Result<String, String> {
        let id = control(arguments)?;
        let capabilities = self.camera.capabilities().map_err(|error| error.to_string())?;
        let capability = capabilities
            .iter()
            .find(|capability| capability.id == id)
            .ok_or(format!("the camera has no {id}"))?;
        let factory = ControlSetting::manual(capability.range.default());
        self.camera.write(id, factory).map_err(|error| error.to_string())?;
        Ok(describe(id, factory))
    }

    fn apply_profile(&mut self, arguments: &Value) -> Result<String, String> {
        let profiles = self.profiles.as_mut().ok_or("no profiles are saved for this camera")?;
        if let Some(name) = arguments.get("name").and_then(Value::as_str) {
            profiles.select(name).map_err(|error| error.to_string())?;
        }
        let profile = profiles.active();
        let report = apply(self.camera, profile).map_err(|error| error.to_string())?;
        Ok(format!("applied `{}` to {} controls", profile.name(), report.applied.len()))
    }
}

fn control(arguments: &Value) -> Result<ControlId, String> {
    let name = arguments.get("control").and_then(Value::as_str).ok_or("`control` is required")?;
    name.parse().map_err(|error: crate::UnknownControl| error.to_string())
}

fn describe(id: ControlId, setting: ControlSetting) -> String {
    format!("{id} = {}{}", setting.value, if setting.is_auto() { " (auto)" } else { "" })
}

fn initialize(params: &Value) -> Value {
    let protocol = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(LATEST_PROTOCOL);
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "webca", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn tools() -> Value {
    let control = json!({ "type": "string", "description": "Control key, for example `brightness` or `white_balance`" });
    json!([
        { "name": "list_controls", "description": "List every camera control with its range and current value",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "get_control", "description": "Read the current value of one control",
          "inputSchema": { "type": "object", "properties": { "control": control }, "required": ["control"] } },
        { "name": "set_control", "description": "Change a control right now; use \"auto\" to let the camera decide",
          "inputSchema": { "type": "object", "properties": {
              "control": control,
              "value": { "oneOf": [{ "type": "integer" }, { "type": "string", "enum": ["auto"] }] } },
            "required": ["control", "value"] } },
        { "name": "reset_control", "description": "Put a control back to its factory value",
          "inputSchema": { "type": "object", "properties": { "control": control }, "required": ["control"] } },
        { "name": "list_profiles", "description": "List saved profiles; the active one is marked with *",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "apply_profile", "description": "Apply the active or the named saved profile to the camera",
          "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } } } },
    ])
}

fn success(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn failure(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DeviceError,
        testing::{FakeCamera, sample_capabilities},
    };

    fn request(server: &mut McpServer<'_>, method: &str, params: &Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }).to_string();
        serde_json::from_str(&server.handle(&line).expect("requests get responses")).unwrap()
    }

    fn call(server: &mut McpServer<'_>, tool: &str, arguments: &Value) -> Value {
        request(server, "tools/call", &json!({ "name": tool, "arguments": arguments }))["result"].clone()
    }

    fn text(result: &Value) -> &str {
        result["content"][0]["text"].as_str().unwrap()
    }

    fn profiles() -> CameraProfiles {
        let mut profiles = CameraProfiles::default();
        profiles.add("Night").unwrap();
        let night = profiles.select("Night").unwrap().clone();
        let _ = night;
        profiles.active_mut().set(ControlId::Contrast, ControlSetting::manual(40));
        profiles.select("Default").unwrap();
        profiles
    }

    #[test]
    fn handshake_and_discovery() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, None);
        let init = request(&mut server, "initialize", &json!({ "protocolVersion": "2025-03-26" }));
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "webca");
        assert_eq!(
            request(&mut server, "initialize", &Value::Null)["result"]["protocolVersion"],
            LATEST_PROTOCOL
        );
        assert_eq!(request(&mut server, "ping", &Value::Null)["result"], json!({}));
        let tools = request(&mut server, "tools/list", &Value::Null);
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 6);
        assert_eq!(server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
    }

    #[test]
    fn protocol_errors() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, None);
        let parse: Value = serde_json::from_str(&server.handle("{nope").unwrap()).unwrap();
        assert_eq!(parse["error"]["code"], PARSE_ERROR);
        let missing: Value = serde_json::from_str(&server.handle(r#"{"id":1}"#).unwrap()).unwrap();
        assert_eq!(missing["error"]["code"], INVALID_REQUEST);
        assert_eq!(request(&mut server, "resources/list", &Value::Null)["error"]["code"], METHOD_NOT_FOUND);
        let unnamed = request(&mut server, "tools/call", &json!({}));
        assert_eq!(unnamed["error"]["message"], "missing tool name");
        let unknown = request(&mut server, "tools/call", &json!({ "name": "fly" }));
        assert_eq!(unknown["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn reads_and_changes_controls() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, None);
        assert!(text(&call(&mut server, "list_controls", &json!({}))).contains("brightness"));
        assert_eq!(text(&call(&mut server, "get_control", &json!({ "control": "gamma" }))), "gamma = 5");
        let set = call(&mut server, "set_control", &json!({ "control": "contrast", "value": 150 }));
        assert_eq!(text(&set), "contrast = 150");
        let auto = call(&mut server, "set_control", &json!({ "control": "exposure", "value": "auto" }));
        assert_eq!(text(&auto), "exposure = -6 (auto)");
        assert_eq!(
            text(&call(&mut server, "reset_control", &json!({ "control": "contrast" }))),
            "contrast = 128"
        );
    }

    #[test]
    fn tool_failures_are_reported_as_tool_errors() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, None);
        let cases = [
            ("get_control", json!({})),
            ("get_control", json!({ "control": "iris" })),
            ("get_control", json!({ "control": "focus" })),
            ("set_control", json!({ "control": "gamma" })),
            ("set_control", json!({ "value": 1 })),
            ("reset_control", json!({})),
            ("set_control", json!({ "control": "gamma", "value": 1.5 })),
            ("set_control", json!({ "control": "gamma", "value": "auto" })),
            ("set_control", json!({ "control": "focus", "value": "auto" })),
            ("reset_control", json!({ "control": "focus" })),
            ("apply_profile", json!({})),
        ];
        for (tool, arguments) in cases {
            assert_eq!(call(&mut server, tool, &arguments)["isError"], true, "{tool} {arguments}");
        }
        assert_eq!(text(&call(&mut server, "list_profiles", &Value::Null)), "");
    }

    #[test]
    fn device_failures_surface_in_every_tool() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, Some(profiles()));
        camera.fail_writes(ControlId::Gamma, DeviceError::Busy);
        camera.fail_reads(ControlId::Gamma, DeviceError::Busy);
        assert_eq!(
            call(&mut server, "set_control", &json!({ "control": "gamma", "value": 2 }))["isError"],
            true
        );
        camera.fail_reads(ControlId::Contrast, DeviceError::Busy);
        assert_eq!(
            call(&mut server, "set_control", &json!({ "control": "contrast", "value": 2 }))["isError"],
            true
        );
        assert_eq!(call(&mut server, "reset_control", &json!({ "control": "gamma" }))["isError"], true);
        camera.disconnect();
        assert_eq!(call(&mut server, "list_controls", &json!({}))["isError"], true);
        assert_eq!(call(&mut server, "reset_control", &json!({ "control": "gamma" }))["isError"], true);
        assert_eq!(call(&mut server, "apply_profile", &json!({}))["isError"], true);
    }

    #[test]
    fn applies_saved_profiles() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut server = McpServer::new(&camera, Some(profiles()));
        assert_eq!(text(&call(&mut server, "list_profiles", &json!({}))), "* Default\n  Night\n");
        let applied = call(&mut server, "apply_profile", &json!({ "name": "Night" }));
        assert_eq!(text(&applied), "applied `Night` to 1 controls");
        assert_eq!(camera.read(ControlId::Contrast), Ok(ControlSetting::manual(40)));
        assert_eq!(call(&mut server, "apply_profile", &json!({ "name": "Ghost" }))["isError"], true);
        assert_eq!(text(&call(&mut server, "apply_profile", &json!({}))), "applied `Night` to 1 controls");
    }
}
