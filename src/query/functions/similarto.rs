use super::*;

fn similarto(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_body SIMILAR TO ");
    query.binding(bindings, value);
    Ok(())
}

function!("similarto", similarto);
