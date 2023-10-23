use chrono::NaiveDate;
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct Message {
    pub id: i32,
    pub time: chrono::NaiveDateTime,
    pub author: String,
    pub body: String,
    pub offset: i32,
}

impl From<Message> for MessageTemplate {
    fn from(value: Message) -> Self {
        let time = value.time.time();
        MessageTemplate {
            id: value.id,
            time: Some(time),
            author: value.author,
            body: value.body,
            offset: value.offset,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct MessageTemplate {
    pub id: i32,
    pub time: Option<chrono::NaiveTime>,
    pub author: String,
    pub body: String,
    pub offset: i32,
}

#[derive(Serialize, Debug, Clone)]
pub struct MessageResults {
    pub date: NaiveDate,
    pub messages: Vec<MessageTemplate>,
}

#[derive(Serialize, Debug, Clone)]
pub struct MessageDate {
    pub dates: Option<chrono::NaiveDate>,
}
