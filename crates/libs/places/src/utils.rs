use std::collections::BTreeMap;

pub fn get_country_code(codes: &BTreeMap<String, String>) -> Option<String> {
    codes.get("ISO3166-1:alpha2").cloned()
}

// This function reformat the id by removing spaces, and prepending a prefix
pub fn normalize_id(prefix: &str, id: &str) -> String {
    match prefix {
        "stop_area" => format!(
            "{}:{}",
            prefix,
            &id.replacen("StopArea:", "", 1).replace(' ', "")
        ),
        _ => format!("{}:{}", prefix, &id.replace(' ', "")),
    }
}
