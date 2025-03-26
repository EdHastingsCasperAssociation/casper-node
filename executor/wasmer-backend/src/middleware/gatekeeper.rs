use wasmer::{FunctionMiddleware, MiddlewareError, ModuleMiddleware};

const MIDDLEWARE_NAME: &str = "Gatekeeper";
const FLOATING_POINTS_NOT_ALLOWED: &str = "Floating point opcodes are not allowed";

#[inline]
fn extension_not_allowed_error(extension: &str) -> MiddlewareError {
    MiddlewareError::new(
        MIDDLEWARE_NAME,
        format!("Wasm `{}` extension is not allowed", extension),
    )
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct GatekeeperConfig {
    /// Allow the `bulk_memory` proposal.
    bulk_memory: bool,
    /// Allow the `exceptions` proposal.
    exceptions: bool,
    /// Allow the `function_references` proposal.
    function_references: bool,
    /// Allow the `gc` proposal.
    gc: bool,
    /// Allow the `legacy_exceptions` proposal.
    #[allow(dead_code)]
    legacy_exceptions: bool,
    /// Allow the `memory_control` proposal.
    memory_control: bool,
    /// Allow the `mvp` proposal.
    mvp: bool,
    /// Allow the `reference_types` proposal.
    reference_types: bool,
    /// Allow the `relaxed_simd` proposal.
    relaxed_simd: bool,
    /// Allow the `saturating_float_to_int` proposal.
    ///
    /// This *requires* canonicalized NaNs enabled in the compiler config.
    saturating_float_to_int: bool,
    /// Allow the `shared_everything_threads` proposal.
    #[allow(dead_code)]
    shared_everything_threads: bool,
    /// Allow the `sign_extension` proposal.
    sign_extension: bool,
    /// Allow the `simd` proposal.
    simd: bool,
    /// Allow the `stack_switching` proposal.
    #[allow(dead_code)]
    stack_switching: bool,
    /// Allow the `tail_call` proposal.
    tail_call: bool,
    /// Allow the `threads` proposal.
    threads: bool,
    /// Allow the `wide_arithmetic` proposal.
    #[allow(dead_code)]
    wide_arithmetic: bool,
    /// Allow floating point opcodes from `mvp` extension.
    ///
    /// This *requires* canonicalized NaNs enabled in the compiler config.
    allow_floating_points: bool,
}

/// Check if the operator is a floating point operator.
#[inline]
const fn is_floating_point(operator: &wasmer::wasmparser::Operator<'_>) -> bool {
    use wasmer::wasmparser::Operator::*;
    match operator {
    // mvp
    F32Load {..} |
    F64Load {..} |
    F32Store {..} |
    F64Store {..} |
    F32Const {..} |
    F64Const {..} |
    F32Abs |
    F32Neg |
    F32Ceil |
    F32Floor |
    F32Trunc |
    F32Nearest |
    F32Sqrt |
    F32Add |
    F32Sub |
    F32Mul |
    F32Div |
    F32Min |
    F32Max |
    F32Copysign |
    F64Abs |
    F64Neg |
    F64Ceil |
    F64Floor |
    F64Trunc |
    F64Nearest |
    F64Sqrt |
    F64Add |
    F64Sub |
    F64Mul |
    F64Div |
    F64Min |
    F64Max |
    F64Copysign |
    F32Eq |
    F32Ne |
    F32Lt |
    F32Gt |
    F32Le |
    F32Ge |
    F64Eq |
    F64Ne |
    F64Lt |
    F64Gt |
    F64Le |
    F64Ge |
    I32TruncF32S |
    I32TruncF32U |
    I32TruncF64S |
    I32TruncF64U |
    I64TruncF32S |
    I64TruncF32U |
    I64TruncF64S |
    I64TruncF64U |
    F32ConvertI32S |
    F32ConvertI32U |
    F32ConvertI64S |
    F32ConvertI64U |
    F32DemoteF64 |
    F64ConvertI32S |
    F64ConvertI32U |
    F64ConvertI64S |
    F64ConvertI64U |
    F64PromoteF32 |
    I32ReinterpretF32 |
    I64ReinterpretF64 |
    F32ReinterpretI32 |
    F64ReinterpretI64 |
    // saturating_float_to_int
    I32TruncSatF32S |
    I32TruncSatF32U |
    I32TruncSatF64S |
    I32TruncSatF64U |
    I64TruncSatF32S |
    I64TruncSatF32U |
    I64TruncSatF64S |
    I64TruncSatF64U |
    // simd
    F32x4ExtractLane{..} |
    F32x4ReplaceLane{..} |
    F64x2ExtractLane{..} |
    F64x2ReplaceLane{..} |
    F32x4Splat |
    F64x2Splat |
    F32x4Eq |
    F32x4Ne |
    F32x4Lt |
    F32x4Gt |
    F32x4Le |
    F32x4Ge |
    F64x2Eq |
    F64x2Ne |
    F64x2Lt |
    F64x2Gt |
    F64x2Le |
    F64x2Ge |
    F32x4Ceil |
    F32x4Floor |
    F32x4Trunc |
    F32x4Nearest |
    F32x4Abs |
    F32x4Neg |
    F32x4Sqrt |
    F32x4Add |
    F32x4Sub |
    F32x4Mul |
    F32x4Div |
    F32x4Min |
    F32x4Max |
    F32x4PMin |
    F32x4PMax |
    F64x2Ceil |
    F64x2Floor |
    F64x2Trunc |
    F64x2Nearest |
    F64x2Abs |
    F64x2Neg |
    F64x2Sqrt |
    F64x2Add |
    F64x2Sub |
    F64x2Mul |
    F64x2Div |
    F64x2Min |
    F64x2Max |
    F64x2PMin |
    F64x2PMax |
    I32x4TruncSatF32x4S |
    I32x4TruncSatF32x4U |
    F32x4ConvertI32x4S |
    F32x4ConvertI32x4U |
    I32x4TruncSatF64x2SZero |
    I32x4TruncSatF64x2UZero |
    F64x2ConvertLowI32x4S |
    F64x2ConvertLowI32x4U |
    F32x4DemoteF64x2Zero |
    F64x2PromoteLowF32x4 |
    // relaxed_simd extension
    I32x4RelaxedTruncF32x4S |
    I32x4RelaxedTruncF32x4U |
    I32x4RelaxedTruncF64x2SZero |
    I32x4RelaxedTruncF64x2UZero |
    F32x4RelaxedMadd |
    F32x4RelaxedNmadd |
    F64x2RelaxedMadd |
    F64x2RelaxedNmadd |
    F32x4RelaxedMin |
    F32x4RelaxedMax |
    F64x2RelaxedMin |
    F64x2RelaxedMax => true,
    _ => false,
    }
}

impl Default for GatekeeperConfig {
    fn default() -> Self {
        Self {
            bulk_memory: false,
            exceptions: false,
            function_references: false,
            gc: false,
            legacy_exceptions: false,
            memory_control: false,
            mvp: true,
            reference_types: false,
            relaxed_simd: false,
            saturating_float_to_int: false,
            shared_everything_threads: false,
            sign_extension: true,
            simd: false,
            stack_switching: false,
            tail_call: false,
            threads: false,
            wide_arithmetic: false,
            // Not yet ready to enable this; needs updated benchmark to accomodate overhead of
            // canonicalized NaNs and manual validation.
            allow_floating_points: false,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Gatekeeper {
    config: GatekeeperConfig,
}

impl Gatekeeper {
    pub(crate) fn new(config: GatekeeperConfig) -> Self {
        Self { config }
    }
}

impl ModuleMiddleware for Gatekeeper {
    fn generate_function_middleware(
        &self,
        _local_function_index: wasmer::LocalFunctionIndex,
    ) -> Box<dyn wasmer::FunctionMiddleware> {
        Box::new(FunctionGatekeeper::new(self.config))
    }
}

#[derive(Debug)]
struct FunctionGatekeeper {
    config: GatekeeperConfig,
}

impl FunctionGatekeeper {
    fn new(config: GatekeeperConfig) -> Self {
        Self { config }
    }

    /// Ensure that floating point opcodes are allowed.
    fn ensure_floating_point_allowed(
        &self,
        operator: &wasmer::wasmparser::Operator<'_>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if !self.config.allow_floating_points && is_floating_point(operator) {
            return Err(MiddlewareError::new(
                MIDDLEWARE_NAME,
                FLOATING_POINTS_NOT_ALLOWED,
            ));
        }
        Ok(())
    }

    fn validated_push_operator<'b, 'a: 'b>(
        &self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        // This is a late check as we first check if given extension is allowed and then check if
        // floating point opcodes are allowed. This is because different Wasm extensions do
        // contain floating point opcodes and this approach makes all the gatekeeping more robust.
        self.ensure_floating_point_allowed(&operator)?;
        // Push the operator to the state.
        state.push_operator(operator);
        Ok(())
    }

    fn bulk_memory<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.bulk_memory {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("bulk_memory"))
        }
    }

    fn exceptions<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.exceptions {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("exceptions"))
        }
    }

    fn function_references<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.function_references {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("function_references"))
        }
    }

    fn gc<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.gc {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("gc"))
        }
    }

    #[allow(dead_code)]
    fn legacy_exceptions<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.legacy_exceptions {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("legacy_exceptions"))
        }
    }

    fn memory_control<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.memory_control {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("memory_control"))
        }
    }

    fn mvp<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.mvp {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("mvp"))
        }
    }

    fn reference_types<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.reference_types {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("reference_types"))
        }
    }

    fn relaxed_simd<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.relaxed_simd {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("relaxed_simd"))
        }
    }

    fn saturating_float_to_int<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.saturating_float_to_int {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("saturating_float_to_int"))
        }
    }

    #[allow(dead_code)]
    fn shared_everything_threads<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.shared_everything_threads {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("shared_everything_threads"))
        }
    }

    fn sign_extension<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.sign_extension {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("sign_extension"))
        }
    }

    fn simd<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.simd {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("simd"))
        }
    }

    #[allow(dead_code)]
    fn stack_switching<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.stack_switching {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("stack_switching"))
        }
    }

    fn tail_call<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.tail_call {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("tail_call"))
        }
    }
    fn threads<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.threads {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("threads"))
        }
    }

    #[allow(dead_code)]
    fn wide_arithmetic<'b, 'a: 'b>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'b>,
    ) -> Result<(), wasmer::MiddlewareError> {
        if self.config.wide_arithmetic {
            self.validated_push_operator(operator, state)?;
            Ok(())
        } else {
            Err(extension_not_allowed_error("wide_arithmetic"))
        }
    }
}

impl FunctionMiddleware for FunctionGatekeeper {
    fn feed<'a>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'a>,
    ) -> Result<(), wasmer::MiddlewareError> {
        macro_rules! match_op {
            ($op:ident { $($payload:tt)* }) => {
                $op { .. }
            };
            ($op:ident) => {
                $op
            };
        }

        macro_rules! gatekeep {
          ($( @$proposal:ident $op:ident $({ $($payload:tt)* })? => $visit:ident)*) => {{
                use wasmer::wasmparser::Operator::*;
                match operator {
                    $(
                        match_op!($op $({ $($payload)* })?) => self.$proposal(operator, state),
                    )*
                }
            }}
        }

        wasmer::wasmparser::for_each_operator!(gatekeep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use wasmer::{sys::EngineBuilder, CompilerConfig, Module, Singlepass, Store, WasmError};

    #[test]
    fn mvp_opcodes_allowed() {
        let bytecode = wat::parse_str(
            r#"
            (module
                (func (export "add") (param i32 i32) (result i32)
                    local.get 0
                    local.get 1
                    i32.add)

            )
            "#,
        )
        .unwrap();
        let mut gatekeeper = Gatekeeper::default();
        gatekeeper.config.mvp = true;
        let gatekeeper = Arc::new(gatekeeper);
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(gatekeeper);
        let store = Store::new(EngineBuilder::new(compiler_config));
        let _module = Module::new(&store, &bytecode).unwrap();
    }
    #[test]
    fn mvp_opcodes_allowed_without_floating_points() {
        let bytecode = wat::parse_str(
            r#"
            (module
                (func (export "add") (param f32 f32) (result f32)
                    local.get 0
                    local.get 1
                    f32.add)
            )
            "#,
        )
        .unwrap();
        let mut gatekeeper = Gatekeeper::default();
        gatekeeper.config.mvp = true;
        gatekeeper.config.allow_floating_points = false;
        let gatekeeper = Arc::new(gatekeeper);
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(gatekeeper);
        let store = Store::new(EngineBuilder::new(compiler_config));
        let error = Module::new(&store, &bytecode).unwrap_err();
        let middleware = match error {
            wasmer::CompileError::Wasm(WasmError::Middleware(middleware)) => middleware,
            _ => panic!("Expected a middleware error"),
        };
        assert_eq!(middleware.message, FLOATING_POINTS_NOT_ALLOWED);
    }

    #[test]
    fn mvp_opcodes_allowed_with_floating_points() {
        let bytecode = wat::parse_str(
            r#"
            (module
                (func (export "add") (param f32 f32) (result f32)
                    local.get 0
                    local.get 1
                    f32.add)
            )
            "#,
        )
        .unwrap();
        let mut gatekeeper = Gatekeeper::default();
        gatekeeper.config.mvp = true;
        gatekeeper.config.allow_floating_points = true;
        let gatekeeper = Arc::new(gatekeeper);
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(gatekeeper);
        let store = Store::new(EngineBuilder::new(compiler_config));
        let _module = Module::new(&store, &bytecode).unwrap();
    }
    #[test]
    fn mvp_opcodes_not_allowed() {
        let bytecode = wat::parse_str(
            r#"
            (module
                (func (export "add") (param i32 i32) (result i32)
                    local.get 0
                    local.get 1
                    i32.add)
            )
            "#,
        )
        .unwrap();
        let mut gatekeeper = Gatekeeper::default();
        gatekeeper.config.mvp = false;
        let gatekeeper = Arc::new(gatekeeper);
        let mut compiler_config = Singlepass::default();
        compiler_config.push_middleware(gatekeeper);
        let store = Store::new(EngineBuilder::new(compiler_config));
        let error = Module::new(&store, &bytecode).unwrap_err();
        assert_eq!(error.to_string(), "WebAssembly translation error: Error in middleware Gatekeeper: Wasm `mvp` extension is not allowed");
    }
}
