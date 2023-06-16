use serde_json::json;

#[derive(Clone, Debug)]
pub enum UpdateOperation {
    /// Update a field `ident` with given value
    Set { ident: String, value: String },
}

/// Given the list of operations to perform, generate the parts of the documents that must be
/// updated.
pub fn generate_document_parts(ops: Vec<UpdateOperation>) -> serde_json::Value {
    ops.into_iter().fold(json!({}), |mut result, op| {
        match op {
            // Adds the part of the document that must be updated to `result`. For
            // example if at current iteration `result` has this value:
            //   { "properties": { "review": "excellent" } }
            // And `ident` = "properties.image", `value` = "https://foo.jpg", then this
            // will update `result` with this value:
            //   { "properties": { "review": "excellent", "image": "https://foo.jpg" } }
            UpdateOperation::Set { ident, value } => {
                // Get a reference to the position in the JSON where the value must be
                // inserted.
                let target = ident.split('.').fold(&mut result, |curr, key| {
                    if curr.get(key).is_none() {
                        curr[key] = json!({});
                    }

                    &mut curr[key]
                });

                // Update target object with the value, in most cases this is just an
                // empty object that was just created to construct the full path.
                *target = value.into();
            }
        }

        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_with_empty_updates() {
        let doc = generate_document_parts(vec![]);
        assert_eq!(doc, json!({}));
    }

    #[test]
    fn generate_with_merged_updates() {
        let ops = vec![
            UpdateOperation::Set {
                ident: "address.city.postcode".into(),
                value: "95600".into(),
            },
            UpdateOperation::Set {
                ident: "address.city.name".into(),
                value: "Eaubonne".into(),
            },
            UpdateOperation::Set {
                ident: "name".into(),
                value: "townhall".into(),
            },
        ];

        assert_eq!(
            generate_document_parts(ops),
            json!({
                "name": "townhall",
                "address": {
                    "city": {
                        "name": "Eaubonne",
                        "postcode": "95600"
                    }
                }
            })
        )
    }
}
