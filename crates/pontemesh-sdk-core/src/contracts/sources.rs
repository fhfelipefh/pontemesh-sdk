use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    Origin,
    ReplicaEdge,
    Peer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedSource {
    pub id: String,
    pub source_type: SourceType,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    pub priority: u8,
    pub expires_at: String,
    pub available_fragments: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelectionContract {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_fragment_priority")]
    pub fragment_priority: String,
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: i64,
    #[serde(default = "default_true")]
    pub allow_peer_sharing: bool,
    #[serde(default = "default_true")]
    pub allow_replica_edge: bool,
}

pub fn is_expired_utc(expires_at: &str) -> bool {
    let Some(expiry) = parse_utc_seconds(expires_at) else {
        return true;
    };
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return true;
    };
    expiry <= now.as_secs() as i64
}

fn parse_utc_seconds(value: &str) -> Option<i64> {
    let value = value
        .strip_suffix('Z')
        .or_else(|| value.strip_suffix("+00:00"))?;
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    let mut time_parts = time.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second_text = time_parts.next()?;
    let second: u32 = second_text.split('.').next()?.parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = (era * 146_097 + doe - 719_468) as i64;
    Some(days * 86_400 + (hour as i64) * 3_600 + (minute as i64) * 60 + second as i64)
}

impl Default for SourceSelectionContract {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            fragment_priority: default_fragment_priority(),
            failure_threshold: default_failure_threshold(),
            allow_peer_sharing: true,
            allow_replica_edge: true,
        }
    }
}

fn default_strategy() -> String {
    "PEER_REPLICA_ORIGIN".to_string()
}

fn default_fragment_priority() -> String {
    "MANIFEST_ORDER".to_string()
}

fn default_failure_threshold() -> i64 {
    2
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::parse_utc_seconds;

    #[test]
    fn parses_server_utc_timestamps() {
        let expected = parse_utc_seconds("2026-07-17T15:31:04Z").expect("z timestamp");

        assert_eq!(
            parse_utc_seconds("2026-07-17T15:31:04.105528+00:00"),
            Some(expected)
        );
    }
}
