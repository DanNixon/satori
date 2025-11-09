use metrics::Unit;
use reqwest::StatusCode;

const ENDPOINTS: &str = "satori_archiver_endpoints";

pub(super) fn init() {
    metrics::describe_gauge!(ENDPOINTS, Unit::Count, "Total requests per endpoint");
}

pub(crate) fn inc_endpoints_metric(endpoint: &'static str, result: StatusCode) {
    let result = result.as_u16().to_string();
    metrics::counter!(ENDPOINTS, "endpoint" => endpoint, "result" => result).increment(1);
}
