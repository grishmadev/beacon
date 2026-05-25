pub mod functions;
pub mod threads;

/*
*TYPES
* */
pub enum EventCommand {
    AddToRejectList(String),
    RemoveFromRejectList(String),
    ListRejectList,
}
pub enum EventResponse {
    ListRejectList(Vec<String>),
}
