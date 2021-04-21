use pyo3::prelude::*;
use pyo3::{types::PyString, PyIterProtocol, PyGCProtocol, PyVisit, PyTraverseError};
use tantivy as tv;

/// Tantivy Token
#[pyclass]
pub(crate) struct Token {
    inner: &'static tv::tokenizer::Token,
}

#[pymethods]
impl Token {
    #[getter]
    fn get_offset_from(&self) -> PyResult<usize> {
        Ok(self.inner.offset_from)
    }

    #[getter]
    fn get_offset_to(&self) -> PyResult<usize> {
        Ok(self.inner.offset_to)
    }

    #[getter]
    fn position(&self) -> PyResult<usize> {
        Ok(self.inner.position)
    }

    #[getter]
    fn text(&self) -> PyResult<&String> {
        Ok(&self.inner.text)
    }

    #[getter]
    fn position_length(&self) -> PyResult<usize> {
        Ok(self.inner.position_length)
    }
}

/// Tantivy TokenStream
#[pyclass(unsendable, gc)]
pub(crate) struct TokenStream {
    ref_: Py<PyString>,
    s_: &'static str,
    inner: tv::tokenizer::BoxTokenStream<'static>,
}

#[pyproto]
impl PyIterProtocol for TokenStream {
    fn __iter__(pyself: PyRef<Self>) -> PyResult<Py<TokenStream>> {
        Ok(pyself.into())
    }

    fn __next__(mut self_: PyRefMut<Self>) -> PyResult<Option<Token>> {
        match self_.inner.advance() {
            true => Ok(Some(Token {
                inner: unsafe {
                    std::mem::transmute::<
                        &tv::tokenizer::Token,
                        &'static tv::tokenizer::Token,
                    >(self_.inner.token())
                },
            })),
            false => Ok(None),
        }
    }
}

#[pyproto]
impl PyGCProtocol for TokenStream {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.ref_)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        drop(&self.ref_);
    }
}

/// Tantivy Tokenizer
#[pyclass(subclass)]
pub(crate) struct Tokenizer {
    pub(crate) inner: Box<dyn tv::tokenizer::Tokenizer>,
}

impl Clone for Tokenizer {
    fn clone(&self) -> Self {
        return Tokenizer {
            inner: self.inner.box_clone(),
        };
    }
}

#[pymethods]
impl Tokenizer {
    fn token_stream(&mut self, text: &PyString) -> PyResult<TokenStream> {
        let text_: Py<PyString> = text.into();
        Python::with_gil(|py| -> PyResult<TokenStream> {
            match text_.as_ref(py).to_str() {
                Err(err) => Err(err),
                Ok(s_) => unsafe {
                    let s_ = std::mem::transmute::<&str, &'static str>(s_);
                    Ok(TokenStream {
                        ref_: text_,
                        s_: s_,
                        inner: self.inner.token_stream(s_),
                    })
                },
            }
        })
    }
}

fn tokenizer_to_py_object(t: &Box<dyn tv::tokenizer::Tokenizer>) -> PyObject {
    Python::with_gil(|py| -> PyObject {
        if t.is::<tv::tokenizer::NgramTokenizer>() {
            Py::new(
                py,
                PyClassInitializer::from(Tokenizer {
                    inner: t.box_clone(),
                })
                .add_subclass(NgramTokenizer {}),
            )
            .unwrap()
            .to_object(py)
        } else {
            Py::new(
                py,
                Tokenizer {
                    inner: t.box_clone(),
                },
            )
            .unwrap()
            .to_object(py)
        }
    })
}

/// Tantivy NgramTokenizer
#[pyclass(extends=Tokenizer)]
#[derive(Clone)]
pub(crate) struct NgramTokenizer {}

#[pymethods]
impl NgramTokenizer {
    #[new]
    fn new(
        min_gram: usize,
        max_gram: usize,
        prefix_only: bool,
    ) -> (Self, Tokenizer) {
        (
            NgramTokenizer {},
            Tokenizer {
                inner: Box::new(tv::tokenizer::NgramTokenizer::new(
                    min_gram,
                    max_gram,
                    prefix_only,
                )),
            },
        )
    }
}

/// Tantivy TextAnalyzer
#[pyclass]
pub(crate) struct TextAnalyzer {
    inner: tv::tokenizer::TextAnalyzer,
}

#[pymethods]
impl TextAnalyzer {
    #[getter]
    fn get_tokenizer(&self) -> PyResult<PyObject> {
        return Ok(tokenizer_to_py_object(&self.inner.tokenizer));
    }
}

/// Tantivy TokenizerManager
#[pyclass]
pub(crate) struct TokenizerManager {
    pub inner: &'static tv::tokenizer::TokenizerManager,
}

#[pymethods]
impl TokenizerManager {
    fn register(
        &mut self,
        tokenizer_name: &str,
        tokenizer: Tokenizer,
    ) -> PyResult<()> {
        self.inner.register_boxed(
            tokenizer_name,
            tv::tokenizer::TextAnalyzer::from(tokenizer.inner),
        );
        Ok(())
    }

    fn get(&mut self, tokenizer_name: &str) -> PyResult<Option<TextAnalyzer>> {
        let tokenizer = self.inner.get(tokenizer_name);
        match tokenizer {
            Some(inner) => Ok(Some(TextAnalyzer { inner: inner })),
            None => Ok(None),
        }
    }
}
