use super::*;

fn author(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let binding_id = query.binding_id(bindings, value);
    let primary = format!(
        "(SELECT coalesce(\
            (SELECT alias_primary FROM aliases WHERE alias_secondary = ${0}), \
            ${0})\
        )",
        binding_id
    );
    query.sql(format!(
        "msg_author = ${0} \
            OR msg_author = {1} \
            OR msg_author IN (SELECT alias_secondary FROM aliases WHERE alias_primary = {1})",
        binding_id, primary
    ));
    Ok(())
}

function!("author", author);

fn rawauthor(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_author = ");
    query.binding(bindings, value);
    Ok(())
}

function!("raw", rawauthor);
