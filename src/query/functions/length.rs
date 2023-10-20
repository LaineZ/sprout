use super::*;

fn length(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let valid_opers = ["!=", ">=", "<=", "=", "<", ">"];
    for oper in &valid_opers {
        if !value.starts_with(oper) {
            continue;
        }
        query.sql("char_length(msg_body) ");
        query.sql(oper);
        let length = value[oper.len()..]
            .parse::<i32>()
            .context("Invalid integer")?;
        query.binding(bindings, length);
        return Ok(());
    }
    let length = value.parse::<i32>().context("Invalid integer")?;
    query.sql("char_length(msg_body) =");
    query.binding(bindings, length);
    Ok(())
}

function!("length", length);
