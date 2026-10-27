use crate::{
    Classifier, Error, Result,
    model::{Message, ThreadDetail},
};
use chrono::{DateTime, SecondsFormat, Utc};

/// Validate and convert an entire thread before its transaction can begin.
pub fn convert_thread(
    detail: &ThreadDetail,
    rules: &Classifier,
    now: DateTime<Utc>,
) -> Result<Vec<Message>> {
    if detail.id.is_empty() {
        return Err(Error::Invalid("Gmail thread has no ID".into()));
    }
    detail
        .messages
        .iter()
        .map(|message| {
            if message.id.is_empty()
                || (!message.thread_id.is_empty() && message.thread_id != detail.id)
            {
                return Err(Error::Invalid(format!(
                    "invalid message identity in thread {}",
                    detail.id
                )));
            }
            let internal_date = if message.internal_date.is_empty() {
                0
            } else {
                message.internal_date.parse::<i64>().map_err(|_| {
                    Error::Invalid(format!(
                        "message {}: invalid internalDate {:?}",
                        message.id, message.internal_date
                    ))
                })?
            };
            let at = if !message.internal_date.is_empty() {
                DateTime::from_timestamp_millis(internal_date).ok_or_else(|| {
                    Error::Invalid(format!(
                        "message {}: internalDate exceeds supported calendar range",
                        message.id
                    ))
                })?
            } else {
                mailparse::dateparse(message.header("Date"))
                    .ok()
                    .and_then(|s| DateTime::from_timestamp(s, 0))
                    .unwrap_or(now)
            };
            let raw_from = message.header("From").trim();
            let header_bytes = format!("From: {raw_from}\r\n");
            let parsed = mailparse::parse_header(header_bytes.as_bytes())
                .ok()
                .and_then(|(h, _)| mailparse::addrparse_header(&h).ok())
                .and_then(|addresses| addresses.extract_single_info());
            let (from_address, from_name) = parsed.map_or_else(
                || (raw_from.into(), String::new()),
                |a| (a.addr, a.display_name.unwrap_or_default()),
            );
            let classification =
                rules.classify(raw_from, &message.snippet, &message.payload.headers);
            Ok(Message {
                id: message.id.clone(),
                thread_id: detail.id.clone(),
                topic_ids: Vec::new(),
                internal_date,
                date_iso: at.to_rfc3339_opts(SecondsFormat::Secs, true),
                from_address,
                from_name,
                to_address: message.header("To").into(),
                subject: message.header("Subject").into(),
                snippet: message.snippet.clone(),
                body_plain: message.snippet.clone(),
                is_automated: classification.is_automated,
                classification_reason: classification.reason,
                created_at: now,
            })
        })
        .collect()
}
