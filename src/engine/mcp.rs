//! Newline-delimited stdio MCP transport. stdout is reserved for JSON-RPC.
use super::agent::{AgentTools, tool_catalog};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

const MAX_REQUEST_BYTES: usize = 512 * 1024;

pub fn run_stdio() -> io::Result<()> {
    run(
        io::stdin().lock(),
        io::stdout().lock(),
        &mut AgentTools::default(),
    )
}

pub fn run(input: impl BufRead, mut output: impl Write, tools: &mut AgentTools) -> io::Result<()> {
    let mut input = input;
    loop {
        let mut line = Vec::new();
        // Bound allocations even when the peer omits the terminating newline.
        let read = std::io::Read::take(&mut input, MAX_REQUEST_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        if line.len() > MAX_REQUEST_BYTES {
            write_message(
                &mut output,
                &rpc_error(Value::Null, -32600, "request exceeds 512 KiB"),
            )?;
            // A truncated frame cannot be safely treated as another request.
            break;
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(_) => {
                write_message(&mut output, &rpc_error(Value::Null, -32700, "invalid JSON"))?;
                continue;
            }
        };
        if let Some(response) = dispatch(request, tools) {
            write_message(&mut output, &response)?;
        }
    }
    tools.save().map_err(io::Error::other)
}

fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()
}
fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

pub fn dispatch(request: Value, tools: &mut AgentTools) -> Option<Value> {
    let id = request.get("id").cloned();
    if !request.is_object()
        || request["jsonrpc"] != "2.0"
        || !request["method"].is_string()
        || id
            .as_ref()
            .is_some_and(|id| !(id.is_string() || id.is_number() || id.is_null()))
    {
        return Some(rpc_error(
            id.unwrap_or(Value::Null),
            -32600,
            "invalid JSON-RPC request",
        ));
    }
    let method = request["method"].as_str().unwrap_or_default();
    // Notifications never receive responses.
    let id = id?;
    let result = match method {
        "initialize" => {
            let requested = request["params"]["protocolVersion"]
                .as_str()
                .unwrap_or_default();
            let version = match requested {
                "2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25" => requested,
                _ => "2025-06-18",
            };
            json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{
                "name":"termcraft","version":env!("CARGO_PKG_VERSION")},
                "instructions":"For live game-master editing, ask the human to /connect in T chat, use director_connect and director_observe, then director_apply for the pending request_id. Use director_request for direct client intentions. Inspect director_observe for the actual outcome; accepted is not applied. Do not enter_world for live editing. Use world_schema, validate_world and create_world for world generation. Enter a world before playing. Observe, choose bounded actions, and inspect results. World creation uses existing blocks; never claim arbitrary new game mechanics. This prototype has stepped single-player simulation. Leave the world before a human opens it."})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":tool_catalog()}),
        "tools/call" => {
            let Some(name) = request["params"]["name"].as_str() else {
                return Some(rpc_error(id, -32602, "tool name is required"));
            };
            let arguments = request["params"]
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match tools.call(name, arguments) {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_default()}],"structuredContent":value,"isError":false})
                }
                Err(error) => json!({"content":[{"type":"text","text":error}],"isError":true}),
            }
        }
        _ => return Some(rpc_error(id, -32601, "method not found")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worlds::WorldStore;

    #[test]
    fn notifications_parse_errors_and_tool_errors_keep_stdio_usable() {
        let input = b"not json\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"observe\",\"arguments\":{}}}\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n";
        let mut output = Vec::new();
        let mut tools = AgentTools::new(WorldStore::new(
            std::env::temp_dir().join("termcraft-mcp-no-writes"),
        ));
        run(&input[..], &mut output, &mut tools).unwrap();
        let messages: Vec<Value> = output
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0]["error"]["code"], -32700);
        assert!(messages[1]["result"]["tools"].as_array().unwrap().len() >= 10);
        assert_eq!(messages[2]["result"]["isError"], true);
        assert_eq!(messages[3]["id"], 3);
    }
}
