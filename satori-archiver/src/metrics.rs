use metrics::Unit;

pub(crate) const ENDPOINTS: &str = "satori_archiver_endpoints";

pub(super) fn init() {
    metrics::describe_gauge!(ENDPOINTS, Unit::Count, "Total requests per endpoint");
}
