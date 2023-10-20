use super::*;

fn regex(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_body ~ ");
    query.binding(bindings, value);
    Ok(())
}

function!("regex", regex);

fn iregex(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_body ~* ");
    query.binding(bindings, value);
    Ok(())
}

function!("iregex", iregex);
