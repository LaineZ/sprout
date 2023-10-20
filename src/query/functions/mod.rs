macro_rules! function {
    ($name:literal, $func:ident) => {
        inventory::submit!(SearchFunction {
            name: $name,
            handler: $func
        });
    };
}

mod author;
mod channel;
mod contains;
mod datetime;
mod length;
mod like;
mod regex;
mod similarto;

use super::{Bindings, QueryBuilder, Result};
use anyhow::{anyhow, Context};

struct SearchFunction {
    name: &'static str,
    handler: fn(&mut QueryBuilder, &mut Bindings, String) -> Result<()>,
}

inventory::collect!(SearchFunction);

pub fn handle(
    query: &mut QueryBuilder,
    bindings: &mut Bindings,
    key: String,
    value: String,
) -> Result<()> {
    for func in inventory::iter::<SearchFunction> {
        if key.eq_ignore_ascii_case(func.name) {
            return (func.handler)(query, bindings, value);
        }
    }

    Err(anyhow!("unknown function '{}'", key))
}
