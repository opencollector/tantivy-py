use crate::to_pyerr;
use pyo3::prelude::*;
use pyo3::types::PyString;
use tantivy as tv;
use tantivy_tokenizer_api as tokenizer_api;

/// Tantivy Token
#[pyclass(module = "tantivy.tantivy")]
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
#[pyclass(module = "tantivy.tantivy", unsendable)]
pub(crate) struct TokenStream {
    inner: tv::tokenizer::BoxTokenStream<'static>,
}

#[pymethods]
impl TokenStream {
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

impl tokenizer_api::Tokenizer for Box<dyn BoxableTokenizer> {
    type TokenStream<'a> = tokenizer_api::BoxTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        (**self).box_token_stream(text)
    }
}

impl Clone for Box<dyn BoxableTokenizer> {
    fn clone(&self) -> Self {
        (**self).box_clone()
    }
}

/// A boxable `Tokenizer`, with its `TokenStream` type erased.
trait BoxableTokenizer: 'static + Send + Sync {
    fn box_token_stream<'a>(
        &'a mut self,
        text: &'a str,
    ) -> tokenizer_api::BoxTokenStream<'a>;
    fn box_clone(&self) -> Box<dyn BoxableTokenizer>;
}

impl<T: tokenizer_api::Tokenizer> BoxableTokenizer for T {
    fn box_token_stream<'a>(
        &'a mut self,
        text: &'a str,
    ) -> tokenizer_api::BoxTokenStream<'a> {
        tokenizer_api::BoxTokenStream::new(self.token_stream(text))
    }

    fn box_clone(&self) -> Box<dyn BoxableTokenizer> {
        Box::new(self.clone())
    }
}

/// Tantivy Tokenizer
#[pyclass(module = "tantivy.tantivy", subclass)]
pub(crate) struct Tokenizer {
    inner: Box<dyn BoxableTokenizer>,
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
    fn token_stream(
        &mut self,
        text: &Bound<PyString>,
    ) -> PyResult<TokenStream> {
        match text.to_str() {
            Err(err) => Err(err),
            Ok(s_) => unsafe {
                Ok(TokenStream {
                    inner: std::mem::transmute::<
                        tv::tokenizer::BoxTokenStream,
                        tv::tokenizer::BoxTokenStream<'static>,
                    >(
                        self.inner.box_token_stream(s_)
                    ),
                })
            },
        }
    }

    #[staticmethod]
    fn create_ngram_tokenizer(
        min_gram: usize,
        max_gram: usize,
        prefix_only: bool,
    ) -> PyResult<Tokenizer> {
        Ok(Tokenizer {
            inner: Box::new(
                tv::tokenizer::NgramTokenizer::new(
                    min_gram,
                    max_gram,
                    prefix_only,
                )
                .map_err(to_pyerr)?,
            ),
        })
    }
}

/// Tantivy TextAnalyzer
#[pyclass(module = "tantivy.tantivy")]
pub(crate) struct TextAnalyzer {
    inner: tv::tokenizer::TextAnalyzer,
}

#[pymethods]
impl TextAnalyzer {
    fn token_stream(
        &mut self,
        texts: &Bound<PyString>,
    ) -> PyResult<TokenStream> {
        Ok(TokenStream {
            inner: unsafe {
                std::mem::transmute::<
                    tv::tokenizer::BoxTokenStream,
                    tv::tokenizer::BoxTokenStream<'static>,
                >(self.inner.token_stream(texts.to_str()?))
            },
        })
    }
}

/// Tantivy TokenizerManager
#[pyclass(module = "tantivy.tantivy")]
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
        self.inner.register(
            tokenizer_name,
            tv::tokenizer::TextAnalyzer::from(tokenizer.inner),
        );
        Ok(())
    }

    fn get(&mut self, tokenizer_name: &str) -> PyResult<Option<TextAnalyzer>> {
        let tokenizer = self.inner.get(tokenizer_name);
        match tokenizer {
            Some(inner) => Ok(Some(TextAnalyzer { inner })),
            None => Ok(None),
        }
    }
}
