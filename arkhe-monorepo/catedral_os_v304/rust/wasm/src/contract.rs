use std::collections::HashMap;
use tracing::info;
use wasmtime::*;

#[derive(Debug, Clone)]
pub struct ContractMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
}

pub struct WasmSandbox {
    engine: Engine,
    linker: Linker<ServiceData>,
    contracts: HashMap<String, Module>,
}

impl WasmSandbox {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .epoch_interruption(true)
            .memory_init_cow(true);

        let engine = Engine::new(&config)?;
        let linker = Linker::new(&engine);

        Ok(Self {
            engine,
            linker,
            contracts: HashMap::new(),
        })
    }

    pub fn load_contract(
        &mut self,
        id: &str,
        wasm_bytes: &[u8],
        meta: ContractMeta,
    ) -> Result<(), String> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Module compilation failed: {}", e))?;
        self.contracts.insert(id.to_string(), module);
        info!("Contract loaded: {} v{}", meta.name, meta.version);
        Ok(())
    }

    pub fn execute(
        &self,
        id: &str,
        func: &str,
        fuel_limit: u64,
    ) -> Result<Vec<u8>, String> {
        let module = self.contracts
            .get(id)
            .ok_or_else(|| format!("Contract not found: {}", id))?;

        let mut store = Store::new(&self.engine, ServiceData::default());
        store.limiter(|data| &mut data.limiter);
        store.set_fuel(fuel_limit).map_err(|e| e.to_string())?;

        let instance = self.linker
            .instantiate(&mut store, module)
            .map_err(|e| format!("Instantiation failed: {}", e))?;

        let func = instance
            .get_func(&mut store, func)
            .ok_or_else(|| format!("Function not found: {}", func))?;

        let mut results = [Val::I32(0)];
        func.call(&mut store, &[], &mut results)
            .map_err(|e| format!("Execution failed: {}", e))?;

Ok(match results[0] {
            Val::I32(v) => v.to_le_bytes().to_vec(),
            Val::I64(v) => v.to_le_bytes().to_vec(),
            Val::F32(f) => f.to_le_bytes().to_vec(),
            Val::F64(f) => f.to_le_bytes().to_vec(),
            ref other => format!("{:?}", other).into_bytes(),
        })
    }

    pub fn list_contracts(&self) -> Vec<&str> {
        self.contracts.keys().map(|s| s.as_str()).collect()
    }

    pub fn unload_contract(&mut self, id: &str) -> bool {
        self.contracts.remove(id).is_some()
    }
}

pub struct ContractRegistry {
    sandbox: WasmSandbox,
    metadata: HashMap<String, ContractMeta>,
}

impl ContractRegistry {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            sandbox: WasmSandbox::new()?,
            metadata: HashMap::new(),
        })
    }

    pub fn register(
        &mut self,
        id: &str,
        wasm_bytes: &[u8],
        meta: ContractMeta,
    ) -> Result<(), String> {
        self.sandbox.load_contract(id, wasm_bytes, meta.clone())?;
        self.metadata.insert(id.to_string(), meta);
        Ok(())
    }

    pub fn execute(&self, id: &str, func: &str) -> Result<Vec<u8>, String> {
        self.sandbox.execute(id, func, 1_000_000)
    }

    pub fn get_meta(&self, id: &str) -> Option<&ContractMeta> {
        self.metadata.get(id)
    }
}

struct StoreLimits {}

impl StoreLimits {
    fn new() -> Self {
        Self {}
    }
}

#[derive(Default)]
struct ServiceData {
    limiter: StoreLimits,
}

impl Default for StoreLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceLimiter for StoreLimits {
    fn memory_growing(
        &mut self,
        _was: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, Error> {
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _was: u32,
        _desired: u32,
        _maximum: Option<u32>,
    ) -> Result<bool, Error> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = WasmSandbox::new();
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_registry_creation() {
        let registry = ContractRegistry::new();
        assert!(registry.is_ok());
    }
}
