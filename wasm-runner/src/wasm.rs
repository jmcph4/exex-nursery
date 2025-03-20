use eyre::eyre;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    IoView, WasiCtx, WasiCtxBuilder, WasiView, preview1::WasiP1Ctx,
};

use crate::exex::ExecutionRequestEvent;

struct WasiRuntimeContext(pub WasiP1Ctx);

impl IoView for WasiRuntimeContext {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        todo!()
    }
}

impl WasiView for WasiRuntimeContext {
    fn ctx(&mut self) -> &mut WasiCtx {
        todo!()
    }
}

impl WasiRuntimeContext {
    pub fn new() -> Self {
        Self(WasiCtxBuilder::new().build_p1())
    }

    pub fn ctx_mut(&mut self) -> &mut WasiP1Ctx {
        &mut self.0
    }
}

pub fn on_req(req: &ExecutionRequestEvent) -> eyre::Result<()> {
    let engine = Engine::default();
    let mut store: Store<WasiRuntimeContext> =
        Store::new(&engine, WasiRuntimeContext::new());
    let module = Module::from_binary(&engine, &req.code)
        .map_err(|e| eyre!("WASM error: {e:?}"))?;
    let mut linker: Linker<WasiRuntimeContext> = Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| {
        ctx.ctx_mut()
    })
    .map_err(|e| eyre!("WASM error: {e:?}"))?;
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| eyre!("WASM error: {e:?}"))?;

    if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
        start
            .call(&mut store, ())
            .map_err(|e| eyre!("WASM Error: {e:?}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_wasm_code_execution_success() {
        let source = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) "Hello, borker!\n")

  (func $main (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 14))
    (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 0))
    drop
  )
)
            "#;
        let code = wat2wasm(&source).unwrap();
        let actual_res = on_req(&ExecutionRequestEvent { code });
        dbg!(&actual_res);
        assert!(actual_res.is_ok());
    }

    fn wat2wasm(source: &str) -> eyre::Result<Vec<u8>> {
        Ok(wat::parse_str(source)?)
    }
}
