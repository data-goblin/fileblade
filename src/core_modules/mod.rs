pub mod canonical;
pub mod emit;
pub mod frontmatter;
pub mod glob;
pub mod metrics;
pub mod recovery_store;
pub mod skills;
pub mod text;
pub mod watch;

use crate::module_helpers::{CoreRoute, Request};
use crate::{AppError, AppResult};
use serde_json::Value;

pub type CoreHandler = fn(&Request<'_>, &[String]) -> AppResult<Value>;

pub fn registered(route: CoreRoute, method: &str) -> Option<CoreHandler> {
    match route {
        CoreRoute::Skills => skills::handler(method),
        _ => None,
    }
}

pub fn dispatch(
    route: CoreRoute,
    request: &Request<'_>,
    arguments: &[String],
) -> Option<AppResult<Value>> {
    let handler = registered(route, request.method)?;
    Some(handler(request, arguments).and_then(|document| {
        if document.is_object() {
            Ok(document)
        } else {
            Err(AppError::command("helper response must be a JSON object"))
        }
    }))
}
