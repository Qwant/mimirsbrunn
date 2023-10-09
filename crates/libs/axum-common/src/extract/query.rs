use crate::extract::json::Json;
use aide::openapi::Operation;
use aide::operation::{add_parameters, parameters_from_schema, ParamLocation};
use aide::OperationInput;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::Query;
use axum_extra::extract::QueryRejection;
use http::request::Parts;
use http::StatusCode;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationErrors};

pub struct ValidatedQuery<T>(pub T);

pub enum ValidatedQueryRejection {
    Validation(ValidationErrors),
    Query(QueryRejection),
}

impl From<QueryRejection> for ValidatedQueryRejection {
    fn from(rejection: QueryRejection) -> Self {
        Self::Query(rejection)
    }
}

impl From<ValidationErrors> for ValidatedQueryRejection {
    fn from(errors: ValidationErrors) -> Self {
        Self::Validation(errors)
    }
}

impl IntoResponse for ValidatedQueryRejection {
    fn into_response(self) -> Response {
        match self {
            ValidatedQueryRejection::Validation(errors) => {
                let mut response = Json(errors).into_response();
                *response.status_mut() = StatusCode::BAD_REQUEST;
                response
            }
            ValidatedQueryRejection::Query(rejection) => rejection.into_response(),
        }
    }
}

#[async_trait]
impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = ValidatedQueryRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Query(query): Query<T> = Query::from_request_parts(parts, _state).await?;
        query.validate()?;
        Ok(ValidatedQuery(query))
    }
}

impl<T> OperationInput for ValidatedQuery<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut aide::gen::GenContext, operation: &mut Operation) {
        let schema = ctx.schema.subschema_for::<T>().into_object();
        let params = parameters_from_schema(ctx, schema, ParamLocation::Query);
        add_parameters(ctx, operation, params);
    }
}
