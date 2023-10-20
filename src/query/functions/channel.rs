use super::*;

fn channel(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    query.sql("msg_channel = ");
    query.binding(bindings, value);
    Ok(())
}

function!("channel", channel);
