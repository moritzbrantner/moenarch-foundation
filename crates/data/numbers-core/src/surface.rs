//! Library-owned runtime surface for `numbers-core`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, validate_matching_lengths, validate_max_items, OperationId, PackageSurface,
    RuntimeCapabilities, SurfaceError, SurfaceRequest, SurfaceResponse,
};
use serde::Deserialize;

use crate::{histogram, quantile, quartiles, summarize_numbers, HistogramConfig, NumberRange};

const MAX_VALUES: usize = 100_000;
const MAX_HISTOGRAM_BINS: usize = 4_096;
const MAX_QUANTILES: usize = 1_024;

/// Describes the scalar numeric operations exposed by transport wrappers.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            surface_operation(
                "describe",
                "Describe package",
                "Deterministic scalar numeric summaries, quantiles, ranges, and histograms for video-analysis.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "numbers.summary",
                "Number summary",
                "Returns finite/non-finite counts and weighted scalar summary statistics.",
                serde_json::json!({"values": [1.0, 2.0, 3.0, 4.0]}),
            ),
            operation(
                "numbers.histogram",
                "Number histogram",
                "Builds fixed-width histogram bins over finite numeric values.",
                serde_json::json!({"values": [0.0, 0.5, 1.0], "bins": 2}),
            ),
            surface_operation(
                "numbers.quantiles",
                "Number quantiles",
                "Returns interpolated quantiles and quartile summary values.",
                serde_json::json!({"values": [1.0, 2.0, 3.0, 4.0], "quantiles": [0.25, 0.5, 0.75]}),
            ),
        ],
    }
}

fn operation(
    id: &str,
    name: &str,
    description: &str,
    example_request: serde_json::Value,
) -> runtime_core::SurfaceOperation {
    let mut operation = surface_operation(id, name, description, example_request);
    if id == "numbers.summary" {
        runtime_core::attach_landscape_contract(
            &mut operation,
            runtime_core::landscape::LandscapeOperationContract::new(
                runtime_core::landscape::LandscapeFunction::new(
                    "numbers.summary",
                    env!("CARGO_PKG_NAME"),
                )
                .input(runtime_core::landscape::LandscapePort::new(
                    "values",
                    runtime_core::landscape::well_known::vector_vector(),
                ))
                .output(runtime_core::landscape::LandscapePort::new(
                    "summary",
                    runtime_core::landscape::well_known::numbers_summary(),
                )),
            ),
        );
    }
    operation
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "numbers.summary" => summary_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "numbers.histogram" => histogram_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "numbers.quantiles" => quantiles_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        operation => {
            return Err(
                SurfaceError::unsupported_operation(operation, env!("CARGO_PKG_NAME"))
                    .to_error_string(),
            );
        }
    };
    Ok(structured_operation_response(&surface, operation, value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryRequest {
    values: Vec<f64>,
    #[serde(default)]
    weights: Option<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistogramRequest {
    values: Vec<f64>,
    #[serde(default = "default_bins")]
    bins: usize,
    #[serde(default)]
    range: Option<RangeRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuantilesRequest {
    values: Vec<f64>,
    #[serde(default = "default_quantiles")]
    quantiles: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RangeRequest {
    min: f64,
    max: f64,
}

fn summary_value(operation: &str, request: SummaryRequest) -> Result<serde_json::Value, String> {
    validate_max_items(operation, "values", request.values.len(), MAX_VALUES)?;
    if let Some(weights) = request.weights {
        validate_matching_lengths(
            operation,
            "weights",
            weights.len(),
            "values",
            request.values.len(),
        )?;
        let mut stats = crate::RunningStats::new();
        for (value, weight) in request.values.into_iter().zip(weights) {
            stats
                .push_weighted(value, weight)
                .map_err(|error| invalid_request(operation, error.to_string()))?;
        }
        Ok(summary_json(stats.summary()))
    } else {
        Ok(summary_json(summarize_numbers(&request.values)))
    }
}

fn summary_json(summary: crate::NumberSummary) -> serde_json::Value {
    serde_json::json!({
        "count": summary.count,
        "finiteCount": summary.finite_count,
        "nonFiniteCount": summary.non_finite_count,
        "min": summary.min,
        "max": summary.max,
        "sum": summary.sum,
        "mean": summary.mean,
        "variance": summary.variance,
        "stdDev": summary.std_dev,
        "weightSum": summary.weight_sum
    })
}

fn histogram_value(
    operation: &str,
    request: HistogramRequest,
) -> Result<serde_json::Value, String> {
    validate_max_items(operation, "values", request.values.len(), MAX_VALUES)?;
    validate_max_items(operation, "bins", request.bins, MAX_HISTOGRAM_BINS)?;
    let mut config = HistogramConfig::new(request.bins)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    if let Some(range) = request.range {
        config = config.with_range(
            NumberRange::new(range.min, range.max)
                .map_err(|error| invalid_request(operation, error.to_string()))?,
        );
    }
    let histogram = histogram(&request.values, config)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "count": histogram.count,
        "min": histogram.min,
        "max": histogram.max,
        "bins": histogram.bins.into_iter().map(|bin| serde_json::json!({
            "start": bin.start,
            "end": bin.end,
            "count": bin.count
        })).collect::<Vec<_>>()
    }))
}

fn quantiles_value(
    operation: &str,
    request: QuantilesRequest,
) -> Result<serde_json::Value, String> {
    validate_max_items(operation, "values", request.values.len(), MAX_VALUES)?;
    validate_max_items(
        operation,
        "quantiles",
        request.quantiles.len(),
        MAX_QUANTILES,
    )?;
    let quantile_values = request
        .quantiles
        .iter()
        .map(|level| {
            quantile(&request.values, *level)
                .map(|value| serde_json::json!({"quantile": level, "value": value}))
                .map_err(|error| invalid_request(operation, error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let quartiles = quartiles(&request.values)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "quantiles": quantile_values,
        "quartiles": {
            "q1": quartiles.q1,
            "median": quartiles.median,
            "q3": quartiles.q3,
            "iqr": quartiles.iqr
        }
    }))
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

fn default_bins() -> usize {
    10
}

fn default_quantiles() -> Vec<f64> {
    vec![0.25, 0.5, 0.75]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_number_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"numbers.summary".to_string()));
        assert!(ids.contains(&"numbers.histogram".to_string()));
        assert!(ids.contains(&"numbers.quantiles".to_string()));
    }

    #[test]
    fn describe_operation_returns_surface_summary() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("describe"),
            input: serde_json::json!({"includeOperations": true}),
        })
        .expect("describe operation");

        assert_eq!(response.operation.as_str(), "describe");
        assert_eq!(response.value["library"], env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn summary_operation_returns_counts_and_mean() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("numbers.summary"),
            input: serde_json::json!({"values": [1.0, 2.0, 3.0]}),
        })
        .expect("summary operation");

        assert_eq!(response.value["count"], 3);
        assert_eq!(response.value["finiteCount"], 3);
        assert_eq!(response.value["mean"], 2.0);
    }

    #[test]
    fn histogram_operation_returns_bins() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("numbers.histogram"),
            input: serde_json::json!({"values": [0.0, 0.5, 1.0], "bins": 2}),
        })
        .expect("histogram operation");

        assert_eq!(response.value["count"], 3);
        assert_eq!(response.value["bins"][0]["count"], 1);
        assert_eq!(response.value["bins"][1]["count"], 2);
    }

    #[test]
    fn quantiles_operation_returns_interpolated_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("numbers.quantiles"),
            input: serde_json::json!({"values": [1.0, 2.0, 3.0, 4.0], "quantiles": [0.5]}),
        })
        .expect("quantiles operation");

        assert_eq!(response.value["quantiles"][0]["value"], 2.5);
        assert_eq!(response.value["quartiles"]["q1"], 1.75);
    }

    #[test]
    fn invalid_request_parsing_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("numbers.summary"),
            input: serde_json::json!({"values": "not an array"}),
        })
        .expect_err("invalid request");

        assert!(error.contains("invalid request"));
    }

    #[test]
    fn unsupported_operation_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("numbers.missing"),
            input: serde_json::json!({}),
        })
        .expect_err("unsupported operation");

        assert!(error.contains("unsupported operation `numbers.missing`"));
    }
}
