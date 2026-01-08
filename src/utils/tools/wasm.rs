//! WASM tool executor using Extism.

use async_trait::async_trait;
use extism::{Manifest, Plugin, Wasm};
use serde_json::Value;
use std::sync::Mutex;

use super::executor::{ToolContext, ToolError, ToolExecutor};

/// Executor for WASM plugins using Extism
pub struct WasmExecutor {
    name: String,
    plugin: Mutex<Plugin>,
    entrypoint: String,
}

impl WasmExecutor {
    /// Create a new WASM executor from a WASM blob
    ///
    /// # Arguments
    /// * `name` - Tool name for logging
    /// * `wasm_bytes` - The compiled WASM module bytes
    /// * `entrypoint` - The function name to call in the WASM module
    ///
    /// # Errors
    /// Returns `ToolError::WasmError` if the plugin cannot be loaded
    pub fn new(name: String, wasm_bytes: &[u8], entrypoint: String) -> Result<Self, ToolError> {
        let wasm = Wasm::data(wasm_bytes.to_vec());
        let manifest = Manifest::new([wasm]);
        
        let plugin = Plugin::new(&manifest, [], true)
            .map_err(|e| ToolError::WasmError(format!("Failed to load WASM plugin: {e}")))?;
        
        Ok(Self {
            name,
            plugin: Mutex::new(plugin),
            entrypoint,
        })
    }
}

#[async_trait]
impl ToolExecutor for WasmExecutor {
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<Value, ToolError> {
        let input_bytes = serde_json::to_vec(&input)
            .map_err(|e| ToolError::InvalidInput(format!("Failed to serialize input: {e}")))?;
        
        let mut plugin = self.plugin.lock()
            .map_err(|e| ToolError::Internal(format!("Plugin lock poisoned: {e}")))?;
        
        let output = plugin.call::<&[u8], &[u8]>(&self.entrypoint, &input_bytes)
            .map_err(|e| ToolError::ExecutionFailed(format!("WASM execution failed: {e}")))?;
        
        let result: Value = serde_json::from_slice(output)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse WASM output: {e}")))?;
        
        Ok(result)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}
