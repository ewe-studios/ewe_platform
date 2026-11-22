//! Fetch one resource from the public base configured by the browser host.
use crate::OpenFuture;
use crate::{Resource, ResourceError, ResourceName, ResourceReader};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ReadableStreamDefaultReader, Response, Url};

struct BodyReader {
    inner: ReadableStreamDefaultReader,
    finished: bool,
}

impl Drop for BodyReader {
    fn drop(&mut self) {
        if !self.finished {
            let cancellation = self.inner.cancel();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = JsFuture::from(cancellation).await {
                    web_sys::console::debug_1(&error);
                }
            });
        }
        self.inner.release_lock();
    }
}

/// Browser resource location. The base must identify an HTTP directory.
pub struct BrowserReader {
    base: String,
}
impl BrowserReader {
    /// Select the public directory of this application.
    ///
    /// # Errors
    /// Returns an error for a non-HTTP address or a base that is not a directory.
    pub fn new(base: &str) -> Result<Self, ResourceError> {
        let invalid = || ResourceError::Read {
            name: base.into(),
            reason: "expected an HTTP resource base ending in /".into(),
        };
        let url = Url::new(base).map_err(|_| invalid())?;
        if !matches!(url.protocol().as_str(), "https:" | "http:")
            || !url.pathname().ends_with('/')
            || !url.search().is_empty()
            || !url.hash().is_empty()
        {
            return Err(invalid());
        }
        Ok(Self { base: url.href() })
    }
}
impl ResourceReader for BrowserReader {
    fn open(&self, name: ResourceName) -> OpenFuture<'_> {
        Box::pin(async move {
            let address = Url::new_with_base(name.as_str(), &self.base)
                .map_err(|e| ResourceError::read(&name, format!("{e:?}")))?;
            let window = web_sys::window().ok_or(ResourceError::MissingHost)?;
            let result = JsFuture::from(window.fetch_with_str(&address.href()))
                .await
                .map_err(|e| ResourceError::read(&name, format!("{e:?}")))?;
            let response: Response = result
                .dyn_into()
                .map_err(|_| ResourceError::read(&name, "invalid fetch response"))?;
            if response.status() == 404 {
                return Err(ResourceError::missing(&name));
            }
            if !response.ok() {
                return Err(ResourceError::read(
                    &name,
                    format!("HTTP {}", response.status()),
                ));
            }
            let Some(body) = response.body() else {
                return Ok(Box::pin(futures_util::stream::empty()) as Resource);
            };
            let reader: ReadableStreamDefaultReader = body
                .get_reader()
                .dyn_into()
                .map_err(|_| ResourceError::read(&name, "invalid response reader"))?;
            let reader = BodyReader {
                inner: reader,
                finished: false,
            };
            let stream =
                futures_util::stream::try_unfold((reader, name), |(mut reader, name)| async move {
                    let err = |e| ResourceError::read(&name, format!("{e:?}"));
                    let item = JsFuture::from(reader.inner.read()).await.map_err(err)?;
                    if js_sys::Reflect::get(&item, &"done".into())
                        .map_err(err)?
                        .as_bool()
                        == Some(true)
                    {
                        reader.finished = true;
                        drop(reader);
                        return Ok(None);
                    }
                    let bytes = js_sys::Reflect::get(&item, &"value".into())
                        .map_err(err)?
                        .dyn_into::<js_sys::Uint8Array>()
                        .map_err(err)?
                        .to_vec();
                    Ok(Some((bytes, (reader, name))))
                });
            Ok(Box::pin(stream) as Resource)
        })
    }
}

/// The resource context installed by a JavaScript application host.
#[cfg(feature = "browser-bindings")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct BrowserResources(crate::Resources);

#[cfg(feature = "browser-bindings")]
#[wasm_bindgen::prelude::wasm_bindgen]
impl BrowserResources {
    /// Install a reader for the application public base.
    ///
    /// # Errors
    /// Returns the host setup error when the base or browser context is unavailable.
    #[wasm_bindgen::prelude::wasm_bindgen(constructor)]
    pub fn new(base: &str) -> Result<Self, wasm_bindgen::JsValue> {
        let ctx = foundation_context::Ctx::try_host()
            .map_err(|err| js_sys::Error::new(&err.to_string()))?;
        crate::Resources::install(
            &ctx,
            std::sync::Arc::new(BrowserReader::new(base).map_err(|error| browser_error(&error))?),
        );
        Ok(Self(crate::Resources::from_context(&ctx)))
    }

    /// Open a resource with the same operation as native application code.
    ///
    /// # Errors
    /// Reports the resource error with its category in the JavaScript error name.
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = "open")]
    pub async fn open_resource(
        &self,
        name: &str,
    ) -> Result<BrowserResourceStream, wasm_bindgen::JsValue> {
        let stream = self
            .0
            .open(name)
            .await
            .map_err(|error| browser_error(&error))?;
        Ok(BrowserResourceStream(futures_util::lock::Mutex::new(
            stream,
        )))
    }
}

/// A browser handle that transfers each chunk on demand.
#[cfg(feature = "browser-bindings")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct BrowserResourceStream(futures_util::lock::Mutex<crate::Resource>);

#[cfg(feature = "browser-bindings")]
#[wasm_bindgen::prelude::wasm_bindgen]
impl BrowserResourceStream {
    /// Return the next chunk, or undefined at the end.
    ///
    /// # Errors
    /// Reports an interrupted or failed body read.
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = nextChunk)]
    pub async fn next_chunk(&self) -> Result<Option<Vec<u8>>, wasm_bindgen::JsValue> {
        use futures_util::StreamExt;
        self.0
            .lock()
            .await
            .next()
            .await
            .transpose()
            .map_err(|error| browser_error(&error))
    }
}

#[cfg(feature = "browser-bindings")]
fn browser_error(error: &ResourceError) -> wasm_bindgen::JsValue {
    let value = js_sys::Error::new(&error.to_string());
    value.set_name(match error {
        ResourceError::MissingHost => "MissingHost",
        ResourceError::InvalidName(_) => "InvalidName",
        ResourceError::Missing { .. } => "Missing",
        ResourceError::Read { .. } => "Read",
    });
    value.into()
}
