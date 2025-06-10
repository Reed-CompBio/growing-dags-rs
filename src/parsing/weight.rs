use super::data::DataFactory;
use anyhow::anyhow;

#[derive(Clone, Copy, Debug, Default)]
pub struct Weight(pub f64);

pub struct WeightDataFactory;
impl DataFactory<Weight> for WeightDataFactory {
    fn len() -> usize {
        1
    }

    fn err_str() -> String {
        "weight".to_string()
    }

    fn from_strs(line: usize, strs: Vec<String>) -> Result<Weight, anyhow::Error> {
        let weight_str = &strs[0];
        str::parse::<f64>(weight_str)
            .map(Weight)
            .map_err(|_| anyhow!("Line {line} has an invalid weight {weight_str}"))
    }
}

pub struct LogWeightDataFactory;
impl DataFactory<Weight> for LogWeightDataFactory {
    fn len() -> usize {
        WeightDataFactory::len()
    }

    fn err_str() -> String {
        WeightDataFactory::err_str()
    }

    fn from_strs(line: usize, strs: Vec<String>) -> Result<Weight, anyhow::Error> {
        let weight = WeightDataFactory::from_strs(line, strs)?;
        // TODO: we use the magic value in Growing DAGs, 0.000000001 (most likely as to make this well-defined at 0,
        // but is there something better here that we can use?)
        Ok(Weight(-f64::ln(
            0.000_000_001_f64.max(weight.0) / f64::ln(10.0),
        )))
    }
}
