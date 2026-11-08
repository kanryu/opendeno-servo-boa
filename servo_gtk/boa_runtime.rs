//! Small application-owned Boa runtime.
//!
//! This is intentionally separate from Servo's page-script runtime.  It lets
//! the GTK application embed and exercise Boa before we replace Servo's
//! SpiderMonkey integration.

use boa_engine::{Context, JsError, JsNativeError, JsString, JsValue, NativeFunction, Source};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// An application-owned JavaScript context backed by Boa.
pub struct BoaRuntime {
    context: Context,
    read_root: Option<PathBuf>,
}

impl Default for BoaRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl BoaRuntime {
    /// Creates a fresh Boa context with the standard ECMAScript built-ins.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: Context::default(),
            read_root: None,
        }
    }

    /// Evaluates one source string and returns its display representation.
    pub fn eval(&mut self, source: &str) -> Result<String, JsError> {
        let value: JsValue = self.context.eval(Source::from_bytes(source))?;
        Ok(value.display().to_string())
    }

    /// Creates a Boa context with a deliberately small, read-only Deno-like
    /// filesystem surface. No filesystem API is exposed by the default
    /// runtime.
    pub fn with_read_only_root(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = fs::canonicalize(root.into()).map_err(|error| error.to_string())?;
        let mut runtime = Self {
            context: Context::default(),
            read_root: Some(root.clone()),
        };

        let read_root = root.clone();
        runtime
            .context
            .register_global_builtin_callable(
                JsString::from("__deno_read_text_file_sync"),
                1,
                // The closure owns an immutable Rust path and does not capture
                // any Boa GC-managed value, so it is safe to keep as a native
                // function for the lifetime of this context.
                unsafe { NativeFunction::from_closure(move |_, args, _| {
                    let name = args
                        .first()
                        .and_then(JsValue::as_string)
                        .ok_or_else(|| JsNativeError::typ().with_message("path must be a string"))?
                        .to_std_string_lossy();
                    let path = read_only_path(&read_root, &name)
                        .map_err(|error| JsNativeError::error().with_message(error))?;
                    let text = fs::read_to_string(path)
                        .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
                    Ok(JsValue::from(JsString::from(text)))
                }) },
            )
            .map_err(|error| error.to_string())?;

        runtime
            .context
            .register_global_builtin_callable(
                JsString::from("__deno_write_text_file_sync"),
                2,
                NativeFunction::from_fn_ptr(|_, _, _| {
                    Err(JsNativeError::error()
                        .with_message("Deno.writeTextFileSync is disabled: read-only sandbox")
                        .into())
                }),
            )
            .map_err(|error| error.to_string())?;

        runtime
            .context
            .eval(Source::from_bytes(
                "globalThis.Deno = Object.freeze({ readTextFileSync: __deno_read_text_file_sync, writeTextFileSync: __deno_write_text_file_sync });",
            ))
            .map_err(|error| error.to_string())?;

        Ok(runtime)
    }

    pub fn read_root(&self) -> Option<&Path> {
        self.read_root.as_deref()
    }
}

fn read_only_path(root: &Path, input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("path must stay inside the read-only root".to_string());
    }
    let candidate = root.join(path);
    let canonical = fs::canonicalize(&candidate).map_err(|error| error.to_string())?;
    if !canonical.starts_with(root) {
        return Err("path escapes the read-only root".to_string());
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::BoaRuntime;

    #[test]
    fn evaluates_javascript_in_an_application_context() {
        let mut runtime = BoaRuntime::new();
        assert_eq!(runtime.eval("1 + 2").unwrap(), "3");
    }

    #[test]
    fn denies_writes_in_read_only_runtime() {
        let mut runtime = BoaRuntime::with_read_only_root(std::env::temp_dir()).unwrap();
        let error = runtime
            .eval("Deno.writeTextFileSync('blocked.txt', 'nope')")
            .unwrap_err();
        assert!(error.to_string().contains("read-only sandbox"));
    }
}
