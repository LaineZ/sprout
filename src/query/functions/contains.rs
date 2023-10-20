use super::*;

fn contains(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let pat = format!("%{}%", value.replace("%", "%%").replace("_", "__"));
    query.sql("msg_body LIKE ");
    query.binding(bindings, pat);
    Ok(())
}

function!("contains", contains);

fn icontains(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let pat = format!("%{}%", value.replace("%", "%%").replace("_", "__"));
    query.sql("msg_body ILIKE ");
    query.binding(bindings, pat);
    Ok(())
}

function!("icontains", icontains);
