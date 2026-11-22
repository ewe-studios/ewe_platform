//! Fetch one resource from the public base configured by the browser host.
use crate::{Resource, ResourceError, ResourceName, ResourceReader};
use futures_util::future::LocalBoxFuture;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ReadableStreamDefaultReader, Response, Url};

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
    fn open<'a>(
        &'a self,
        name: ResourceName,
    ) -> LocalBoxFuture<'a, Result<Resource, ResourceError>> {
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
            let stream =
                futures_util::stream::try_unfold((reader, name), |(reader, name)| async move {
                    let err = |e| ResourceError::read(&name, format!("{e:?}"));
                    let item = JsFuture::from(reader.read()).await.map_err(err)?;
                    if js_sys::Reflect::get(&item, &"done".into())
                        .map_err(err)?
                        .as_bool()
                        == Some(true)
                    {
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
