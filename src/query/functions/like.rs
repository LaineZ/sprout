use super::*;

fn like(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_body LIKE ");
    query.binding(bindings, value);
    Ok(())
}

function!("like", like);

fn ilike(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_body ILIKE ");
    query.binding(bindings, value);
    Ok(())
}

function!("ilike", ilike);
