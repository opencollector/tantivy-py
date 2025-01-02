#![allow(clippy::new_ret_no_self)]

use crate::{document::Document, facet::Facet, query::Query, to_pyerr};
use pyo3::gc::PyVisit;
use pyo3::types::{PyDict, PyList, PyNotImplemented, PySequence, PyTuple};
use pyo3::{
    basic::CompareOp, exceptions::PyValueError, prelude::*, PyTraverseError,
};
use serde::{Deserialize, Serialize};
use tantivy as tv;
use tantivy::aggregation::AggregationCollector;
use tantivy::collector as tvc;
use tantivy::collector::{
    Count, FacetCollector, FruitHandle, MultiCollector, TopDocs,
};
use tantivy::TantivyDocument;
// Bring the trait into scope. This is required for the `to_named_doc` method.
// However, tantivy-py declares its own `Document` class, so we need to avoid
// introduce the `Document` trait into the namespace.
use tantivy::Document as _;

/// Tantivy's Searcher class
///
/// A Searcher is used to search the index given a prepared Query.
#[pyclass(module = "tantivy.tantivy")]
pub(crate) struct Searcher {
    pub(crate) inner: tv::Searcher,
}

#[derive(Clone, Deserialize, FromPyObject, PartialEq, Serialize)]
enum Fruit {
    #[pyo3(transparent)]
    Score(f32),
    #[pyo3(transparent)]
    Order(u64),
}

impl std::fmt::Debug for Fruit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fruit::Score(s) => f.write_str(&format!("{s}")),
            Fruit::Order(o) => f.write_str(&format!("{o}")),
        }
    }
}

impl<'py> IntoPyObject<'py> for Fruit {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(
        self,
        py: Python<'py>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(match self {
            Fruit::Score(s) => s.into_pyobject(py)?.into_any(),
            Fruit::Order(o) => o.into_pyobject(py)?.into_any(),
        })
    }
}

impl<'a, 'py> IntoPyObject<'py> for &'a Fruit {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(
        self,
        py: Python<'py>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(match self {
            Fruit::Score(s) => s.into_pyobject(py)?.into_any(),
            Fruit::Order(o) => o.into_pyobject(py)?.into_any(),
        })
    }
}

#[pyclass(module = "tantivy.tantivy", unsendable)]
pub(crate) struct FacetChildIterator {
    ref_: Option<Py<FacetCounts>>,
    inner: Box<dyn Iterator<Item = (&'static tv::schema::Facet, u64)>>,
}

#[pymethods]
impl FacetChildIterator {
    fn __iter__(pyself: PyRef<Self>) -> PyResult<Py<FacetChildIterator>> {
        Ok(pyself.into())
    }

    fn __next__(mut self_: PyRefMut<Self>) -> PyResult<Option<(Facet, u64)>> {
        Ok(match self_.inner.next() {
            Some((facet, count)) => Some((
                Facet {
                    inner: facet.clone(),
                },
                count,
            )),
            None => None,
        })
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        if let Some(ref ref_) = self.ref_ {
            visit.call(ref_)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.ref_ = None;
    }
}

#[pyclass(module = "tantivy.tantivy")]
pub(crate) struct FacetCounts {
    ref_: Option<Py<SearchResult>>,
    inner: &'static tvc::FacetCounts,
}

#[pymethods]
impl FacetCounts {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        if let Some(ref ref_) = self.ref_ {
            visit.call(ref_)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.ref_ = None;
    }

    fn get(
        self_: Py<FacetCounts>,
        f: &Facet,
        py: Python,
    ) -> FacetChildIterator {
        FacetChildIterator {
            ref_: Some(self_.clone_ref(py)),
            inner: Box::new(self_.borrow(py).inner.get(f.inner.clone())),
        }
    }

    fn top_k(&self, f: &Facet, k: usize) -> Vec<(Facet, u64)> {
        self.inner
            .top_k(f.inner.clone(), k)
            .iter()
            .map(|&(facet, count)| -> (Facet, u64) {
                (
                    Facet {
                        inner: facet.clone(),
                    },
                    count,
                )
            })
            .collect()
    }
}

#[pyclass(frozen, eq, eq_int, module = "tantivy.tantivy")]
#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
/// Enum representing the direction in which something should be sorted.
pub(crate) enum Order {
    /// Ascending. Smaller values appear first.
    Asc,

    /// Descending. Larger values appear first.
    Desc,
}

impl From<Order> for tv::Order {
    fn from(order: Order) -> Self {
        match order {
            Order::Asc => tv::Order::Asc,
            Order::Desc => tv::Order::Desc,
        }
    }
}

#[pyclass(frozen, module = "tantivy.tantivy")]
#[derive(Clone, Default)]
/// Object holding a results successful search.
pub(crate) struct SearchResult {
    hits: Vec<(Fruit, DocAddress)>,
    #[pyo3(get)]
    /// How many documents matched the query. Only available if `count` was set
    /// to true during the search.
    count: Option<usize>,
    /// Facet counts
    facet_axes: Vec<(String, tv::collector::FacetCounts)>,
}

#[pymethods]
impl SearchResult {
    fn __repr__(&self) -> PyResult<String> {
        if let Some(count) = self.count {
            Ok(format!(
                "SearchResult(hits: {:?}, count: {})",
                self.hits, count
            ))
        } else {
            Ok(format!("SearchResult(hits: {:?})", self.hits))
        }
    }

    #[getter]
    /// The list of tuples that contains the scores and DocAddress of the
    /// search results.
    fn hits(&self, py: Python) -> PyResult<Vec<(Py<PyAny>, DocAddress)>> {
        let ret: Vec<(Py<PyAny>, DocAddress)> = self
            .hits
            .iter()
            .map(|(result, address)| {
                Ok((result.into_pyobject(py)?.unbind(), address.clone()))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(ret)
    }

    #[getter]
    fn facet_axes(self_: Py<SearchResult>, py: Python) -> PyResult<Py<PyList>> {
        let result = PyList::empty(py);
        for (field, facet_counts) in &self_.borrow(py).facet_axes {
            let pair: Py<PyTuple> = (
                field.as_str().into_pyobject(py)?.unbind(),
                Py::new(
                    py,
                    FacetCounts {
                        ref_: Some(self_.clone_ref(py)),
                        inner: unsafe {
                            std::mem::transmute::<
                                &tv::collector::FacetCounts,
                                &'static tv::collector::FacetCounts,
                            >(facet_counts)
                        },
                    },
                )?,
            )
                .into_pyobject(py)?
                .unbind();
            result.append(pair)?;
        }
        return Ok(result.into());
    }
}

#[pymethods]
impl Searcher {
    /// Search the index with the given query and collect results.
    ///
    /// Args:
    ///     query (Query): The query that will be used for the search.
    ///     limit (int, optional): The maximum number of search results to
    ///         return. Defaults to 10.
    ///     count (bool, optional): Should the number of documents that match
    ///         the query be returned as well. Defaults to true.
    ///     order_by_field (Field, optional): A schema field that the results
    ///         should be ordered by. The field must be declared as a fast field
    ///         when building the schema. Note, this only works for unsigned
    ///         fields.
    ///     offset (Field, optional): The offset from which the results have
    ///         to be returned.
    ///     facet_axes (&PySequence, optional): Gets the searcher to return the
    ///         specified axes of facets.
    ///     order (Order, optional): The order in which the results
    ///         should be sorted. If not specified, defaults to descending.
    ///
    /// Returns `SearchResult` object.
    ///
    /// Raises a ValueError if there was an error with the search.
    #[pyo3(signature = (query, limit = 10, count = true, order_by_field = None, offset = 0, facet_axes = None, order = Order::Desc))]
    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        py: Python,
        query: &Query,
        limit: usize,
        count: bool,
        order_by_field: Option<&str>,
        offset: usize,
        facet_axes: Option<&Bound<PySequence>>,
        order: Order,
    ) -> PyResult<SearchResult> {
        let mut multicollector = MultiCollector::new();

        let mut facet_counts_handles: Vec<(
            String,
            FruitHandle<tv::collector::FacetCounts>,
        )> = Vec::new();
        if let Some(facet_axes) = facet_axes {
            for i in 0..facet_axes.len()? {
                let axis: (String, Vec<Py<Facet>>) =
                    facet_axes.get_item(i)?.extract()?;
                let field_name = &axis.0;
                let mut facetcollector = FacetCollector::for_field(field_name);
                for facet in &axis.1 {
                    facetcollector.add_facet(facet.borrow(py).inner.clone())
                }
                facet_counts_handles.push((
                    field_name.to_owned(),
                    multicollector.add_collector(facetcollector),
                ));
            }
        }

        py.allow_threads(move || {
            let count_handle = if count {
                Some(multicollector.add_collector(Count))
            } else {
                None
            };

            let (mut multifruit, hits) = {
                if let Some(order_by) = order_by_field {
                    let collector = TopDocs::with_limit(limit)
                        .and_offset(offset)
                        .order_by_u64_field(order_by, order.into());
                    let top_docs_handle =
                        multicollector.add_collector(collector);
                    let ret = self.inner.search(query.get(), &multicollector);

                    match ret {
                        Ok(mut r) => {
                            let top_docs = top_docs_handle.extract(&mut r);
                            let result: Vec<(Fruit, DocAddress)> = top_docs
                                .iter()
                                .map(|(f, d)| {
                                    (Fruit::Order(*f), DocAddress::from(d))
                                })
                                .collect();
                            (r, result)
                        }
                        Err(e) => {
                            return Err(PyValueError::new_err(e.to_string()))
                        }
                    }
                } else {
                    let collector =
                        TopDocs::with_limit(limit).and_offset(offset);
                    let top_docs_handle =
                        multicollector.add_collector(collector);
                    let ret = self.inner.search(query.get(), &multicollector);

                    match ret {
                        Ok(mut r) => {
                            let top_docs = top_docs_handle.extract(&mut r);
                            let result: Vec<(Fruit, DocAddress)> = top_docs
                                .iter()
                                .map(|(f, d)| {
                                    (Fruit::Score(*f), DocAddress::from(d))
                                })
                                .collect();
                            (r, result)
                        }
                        Err(e) => {
                            return Err(PyValueError::new_err(e.to_string()))
                        }
                    }
                }
            };

            let count = count_handle.map(|h| h.extract(&mut multifruit));

            let mut facet_axes =
                Vec::<(String, tv::collector::FacetCounts)>::new();
            for (field_name, h) in facet_counts_handles {
                facet_axes
                    .push((field_name.to_string(), h.extract(&mut multifruit)))
            }

            Ok(SearchResult {
                hits,
                count,
                facet_axes,
            })
        })
    }

    #[pyo3(signature = (query, agg))]
    fn aggregate(
        &self,
        py: Python,
        query: &Query,
        agg: Py<PyDict>,
    ) -> PyResult<Py<PyDict>> {
        let py_json = py.import("json")?;
        let agg_query_str = py_json.call_method1("dumps", (agg,))?.to_string();

        let agg_str = py.allow_threads(move || {
            let agg_collector = AggregationCollector::from_aggs(
                serde_json::from_str(&agg_query_str).map_err(to_pyerr)?,
                Default::default(),
            );
            let agg_res = self
                .inner
                .search(query.get(), &agg_collector)
                .map_err(to_pyerr)?;

            serde_json::to_string(&agg_res).map_err(to_pyerr)
        })?;

        let agg_dict = py_json.call_method1("loads", (agg_str,))?;
        let agg_dict = agg_dict.downcast::<PyDict>()?;

        Ok(agg_dict.clone().unbind())
    }

    /// Returns the overall number of documents in the index.
    #[getter]
    fn num_docs(&self) -> u64 {
        self.inner.num_docs()
    }

    /// Returns the number of segments in the index.
    #[getter]
    fn num_segments(&self) -> usize {
        self.inner.segment_readers().len()
    }

    /// Return the overall number of documents containing
    /// the given term.
    #[pyo3(signature = (field_name, field_value))]
    fn doc_freq(
        &self,
        field_name: &str,
        field_value: &Bound<PyAny>,
    ) -> PyResult<u64> {
        // Wrap the tantivy Searcher `doc_freq` method to return a PyResult.
        let schema = self.inner.schema();
        let term = crate::make_term(schema, field_name, field_value)?;
        self.inner.doc_freq(&term).map_err(to_pyerr)
    }

    /// Fetches a document from Tantivy's store given a DocAddress.
    ///
    /// Args:
    ///     doc_address (DocAddress): The DocAddress that is associated with
    ///         the document that we wish to fetch.
    ///
    /// Returns the Document, raises ValueError if the document can't be found.
    fn doc(&self, doc_address: &DocAddress) -> PyResult<Document> {
        let doc: TantivyDocument =
            self.inner.doc(doc_address.into()).map_err(to_pyerr)?;
        let named_doc = doc.to_named_doc(self.inner.schema());
        Ok(crate::document::Document {
            field_values: named_doc.0,
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Searcher(num_docs={}, num_segments={})",
            self.inner.num_docs(),
            self.inner.segment_readers().len()
        ))
    }
}

/// DocAddress contains all the necessary information to identify a document
/// given a Searcher object.
///
/// It consists in an id identifying its segment, and its segment-local DocId.
/// The id used for the segment is actually an ordinal in the list of segment
/// hold by a Searcher.
#[pyclass(frozen, module = "tantivy.tantivy")]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct DocAddress {
    pub(crate) segment_ord: tv::SegmentOrdinal,
    pub(crate) doc: tv::DocId,
}

#[pymethods]
impl DocAddress {
    #[new]
    fn new(segment_ord: tv::SegmentOrdinal, doc: tv::DocId) -> Self {
        DocAddress { segment_ord, doc }
    }

    /// The segment ordinal is an id identifying the segment hosting the
    /// document. It is only meaningful, in the context of a searcher.
    #[getter]
    fn segment_ord(&self) -> u32 {
        self.segment_ord
    }

    /// The segment local DocId
    #[getter]
    fn doc(&self) -> u32 {
        self.doc
    }

    fn __richcmp__<'py>(
        &self,
        other: &Self,
        op: CompareOp,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Ok(match op {
            CompareOp::Eq => {
                (self == other).into_pyobject(py)?.to_owned().into_any()
            }
            CompareOp::Ne => {
                (self != other).into_pyobject(py)?.to_owned().into_any()
            }
            _ => PyNotImplemented::get(py).to_owned().into_any(),
        })
    }

    fn __getnewargs__(&self) -> PyResult<(tv::SegmentOrdinal, tv::DocId)> {
        Ok((self.segment_ord, self.doc))
    }
}

impl From<&tv::DocAddress> for DocAddress {
    fn from(val: &tv::DocAddress) -> Self {
        DocAddress {
            segment_ord: val.segment_ord,
            doc: val.doc_id,
        }
    }
}

impl Into<tv::DocAddress> for &DocAddress {
    fn into(self) -> tv::DocAddress {
        tv::DocAddress {
            segment_ord: self.segment_ord,
            doc_id: self.doc,
        }
    }
}
