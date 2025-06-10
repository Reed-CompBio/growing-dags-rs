//! # Data
//!
//! Data can be associated with the nodes in a network, and may
//! have a pre-processing step. While this is only used for Growing DAGs
//! with respect to weight and log-weight, this defines a generic methodology
//! in case someone wants to define their own weight factory.

/// A data factory parses inputs
/// from a list of strings on a specific line of the interactome input,
/// with a guaranteed `Self::len()`. [non-deterministic lengths can lead to panics.]
pub trait DataFactory<D> {
    fn len() -> usize;
    fn err_str() -> String;
    fn from_strs(line: usize, strs: Vec<String>) -> Result<D, anyhow::Error>;
}

pub struct EmptyTupleDataFactory;

impl DataFactory<()> for EmptyTupleDataFactory {
    fn len() -> usize {
        0
    }

    fn err_str() -> String {
        "nothing following".to_string()
    }

    fn from_strs(_line: usize, _strs: Vec<String>) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
