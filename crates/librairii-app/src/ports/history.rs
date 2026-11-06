use super::{creation::InputError, operations::OperationSnapshot};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPage {
    pub operations: Vec<OperationSnapshot>,
    pub next_before: Option<String>,
}
impl HistoryPage {
    pub fn presentation(&self) -> Result<serde_json::Value, InputError> {
        let operations = self
            .operations
            .iter()
            .map(OperationSnapshot::presentation)
            .collect::<Result<Vec<_>, _>>()?;
        let mut value = serde_json::json!({"operations":operations});
        if let Some(before) = &self.next_before {
            value["nextBefore"] = serde_json::json!(before);
        }
        Ok(value)
    }
}
