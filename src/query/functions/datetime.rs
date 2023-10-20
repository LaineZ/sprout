use super::*;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

fn date(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let valid_opers = ["!=", ">=", "<=", "=", "<", ">"];
    for oper in &valid_opers {
        if !value.starts_with(oper) {
            continue;
        }
        let date = value[oper.len()..]
            .parse::<NaiveDate>()
            .context("Invalid date")?;
        query.sql("msg_timestamp::date ");
        query.sql(oper);
        query.binding(bindings, date);
        return Ok(());
    }

    let date = value.parse::<NaiveDate>().context("Invalid date")?;
    query.sql("msg_timestamp::date =");
    query.binding(bindings, date);
    Ok(())
}

function!("date", date);

fn time(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let valid_opers = ["!=", ">=", "<=", "=", "<", ">"];
    for oper in &valid_opers {
        if !value.starts_with(oper) {
            continue;
        }
        let time = value[oper.len()..]
            .parse::<NaiveTime>()
            .context("Invalid time")?;
        query.sql("msg_timestamp::time ");
        query.sql(oper);
        query.binding(bindings, time);
        return Ok(());
    }

    let time = value.parse::<NaiveTime>().context("Invalid time")?;
    query.sql("msg_timestamp::time =");
    query.binding(bindings, time);
    Ok(())
}

function!("time", time);

fn datetime(query: &mut QueryBuilder, bindings: &mut Bindings, value: String) -> Result<()> {
    let valid_opers = ["!=", ">=", "<=", "=", "<", ">"];
    let fmt = "%Y-%m-%d %H:%M:%S";
    for oper in &valid_opers {
        if !value.starts_with(oper) {
            continue;
        }
        let datetime =
            NaiveDateTime::parse_from_str(&value[oper.len()..], fmt).context("Invalid datetime")?;
        query.sql("msg_timestamp ");
        query.sql(oper);
        query.binding(bindings, datetime);
        return Ok(());
    }

    let datetime = NaiveDateTime::parse_from_str(&value, fmt).context("Invalid datetime")?;
    query.sql("msg_timestamp =");
    query.binding(bindings, datetime);
    Ok(())
}

function!("datetime", datetime);
