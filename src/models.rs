use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Message {
    pub time: chrono::NaiveDateTime,
    pub author: String,
    pub body: String,
    pub offset: i32,
}

#[derive(Serialize, Debug)]
pub struct MessageDate {
    pub dates: Option<chrono::NaiveDate>,
}