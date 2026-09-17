use crate::AppResult;
use crate::core_modules::Context;
use crate::core_modules::usage::{self, store::Environment};
use serde_json::{Map, Value};

pub const UNAVAILABLE: &str = usage::UNAVAILABLE;

pub fn attach(context: &Context<'_>, document: &mut Map<String, Value>) {
    if context.check().is_err() {
        document.insert("usageError".to_string(), Value::from(UNAVAILABLE));
        document.insert("usageWatchPaths".to_string(), Value::Array(Vec::new()));
        return;
    }
    usage::attach_skills(&environment(), document);
}

pub fn environment() -> Environment {
    usage::environment()
}

pub fn history(context: &Context<'_>, items: &[Value], scoped: bool) -> AppResult<Value> {
    context.check()?;
    Ok(usage::query::skill_usage(&environment(), items, scoped))
}

pub fn counts(context: &Context<'_>, items: &[Value]) -> AppResult<Value> {
    context.check()?;
    Ok(usage::query::skill_counts(&environment(), items))
}

pub fn day(context: &Context<'_>, items: &[Value], day: &str) -> AppResult<Value> {
    context.check()?;
    Ok(usage::query::skill_day(&environment(), items, day))
}
