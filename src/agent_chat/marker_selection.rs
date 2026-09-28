use matrix_sdk::ruma::serde::Raw;
#[derive(Debug)]
pub(super) struct ApprovalMarkerSelection {
    pub observed_v2: bool,
    pub marker: Result<Option<serde_json::Value>, ()>,
}

struct ApprovalMarkerEnvelope<'a> {
    event_type: std::borrow::Cow<'a, str>,
    state_key: std::borrow::Cow<'a, str>,
    has_single_content: bool,
}

impl<'de> serde::Deserialize<'de> for ApprovalMarkerEnvelope<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ApprovalMarkerEnvelope<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a Matrix state event envelope")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut event_type = None;
                let mut state_key = None;
                let mut content_count = 0_usize;
                while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                    match key.as_ref() {
                        "type" => {
                            if event_type.is_some() {
                                return Err(serde::de::Error::duplicate_field("type"));
                            }
                            event_type = Some(map.next_value()?);
                        }
                        "state_key" => {
                            if state_key.is_some() {
                                return Err(serde::de::Error::duplicate_field("state_key"));
                            }
                            state_key = Some(map.next_value()?);
                        }
                        "content" => {
                            let _: &serde_json::value::RawValue = map.next_value()?;
                            content_count = content_count.saturating_add(1);
                        }
                        _ => {
                            let _: &serde_json::value::RawValue = map.next_value()?;
                        }
                    }
                }
                Ok(ApprovalMarkerEnvelope {
                    event_type: event_type
                        .ok_or_else(|| serde::de::Error::missing_field("type"))?,
                    state_key: state_key
                        .ok_or_else(|| serde::de::Error::missing_field("state_key"))?,
                    has_single_content: content_count == 1,
                })
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

fn select_approval_marker_state(
    events: Vec<(String, String, usize, serde_json::Value)>,
) -> ApprovalMarkerSelection {
    const V1: &str = "com.agentchat.approval.room.v1";
    const V2: &str = "com.agentchat.approval.room.v2";
    const MAX_EVENT_BYTES: usize = 65_536;

    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    for (event_type, state_key, bytes, event) in events {
        if !state_key.is_empty() {
            continue;
        }
        match event_type.as_str() {
            V1 => v1.push((bytes, event)),
            V2 => v2.push((bytes, event)),
            _ => {}
        }
    }
    let observed_v2 = !v2.is_empty();
    let selected = if observed_v2 { v2 } else { v1 };
    let marker = match selected.as_slice() {
        [] => Ok(None),
        [(bytes, event)] if *bytes <= MAX_EVENT_BYTES => Ok(Some(event.clone())),
        _ => Err(()),
    };
    ApprovalMarkerSelection {
        observed_v2,
        marker,
    }
}

pub(super) fn select_approval_marker_raw_state<'a, T: 'a>(
    events: impl IntoIterator<Item = &'a Raw<T>>,
    namespace: super::approval::Namespace,
) -> ApprovalMarkerSelection {
    const V1: &str = "com.agentchat.approval.room.v1";
    const V2: &str = "com.agentchat.approval.room.v2";

    let mut candidates = Vec::new();
    let mut invalid = false;
    for raw in events {
        let raw_json = raw.json().get();
        let envelope = match serde_json::from_str::<ApprovalMarkerEnvelope<'_>>(raw_json) {
            Ok(envelope) => envelope,
            Err(_) => {
                invalid = true;
                continue;
            }
        };
        let Some(suffix) = envelope.event_type.strip_prefix(namespace.base()) else {
            continue;
        };
        let event_type = format!("com.agentchat{suffix}");
        if !matches!(event_type.as_ref(), V1 | V2) {
            continue;
        }
        let state_key = envelope.state_key;
        let event = match raw_json.len() <= 65_536 && envelope.has_single_content {
            true => match serde_json::from_str::<serde_json::Value>(raw_json) {
                Ok(mut event) => {
                    if let Some(content) = event.get_mut("content") {
                        *content = super::approval_runtime::normalize(namespace, content);
                    }
                    event["type"] = serde_json::json!(event_type);
                    event
                }
                Err(_) => {
                    invalid = true;
                    serde_json::Value::Null
                }
            },
            false => {
                invalid = true;
                serde_json::Value::Null
            }
        };
        candidates.push((event_type, state_key.into_owned(), raw_json.len(), event));
    }
    let mut selected = select_approval_marker_state(candidates);
    if invalid {
        selected.marker = Err(());
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn markers_are_isolated_between_namespaces() {
        for source in super::super::approval::Namespace::ALL {
            let event = Raw::<serde_json::Value>::new(&json!({"type":format!("{}.approval.room.v2",source.base()),"state_key":"","content":{"version":2}})).unwrap();
            for selected in super::super::approval::Namespace::ALL {
                let result = select_approval_marker_raw_state([&event], selected);
                assert_eq!(result.observed_v2, source == selected);
                assert_eq!(result.marker.unwrap().is_some(), source == selected);
            }
        }
    }
    #[test]
    fn malformed_or_duplicate_v2_blocks_v1_downgrade_in_every_namespace() {
        for namespace in super::super::approval::Namespace::ALL {
            let v1=Raw::<serde_json::Value>::new(&json!({"type":format!("{}.approval.room.v1",namespace.base()),"state_key":"","content":{"version":1}})).unwrap();
            let v2=Raw::<serde_json::Value>::new(&json!({"type":format!("{}.approval.room.v2",namespace.base()),"state_key":"","content":null})).unwrap();
            let selected = select_approval_marker_raw_state([&v1, &v2, &v2], namespace);
            assert!(selected.observed_v2);
            assert!(selected.marker.is_err());
            let selected = select_approval_marker_raw_state([&v1, &v2], namespace);
            assert!(selected.observed_v2);
            assert_eq!(
                selected.marker.unwrap().unwrap()["type"],
                "com.agentchat.approval.room.v2"
            );
        }
    }
    #[test]
    fn duplicate_content_keys_are_rejected_before_value_parsing() {
        let raw = Raw::<serde_json::Value>::from_json_string(
            r#"{"type":"com.agentchat.approval.room.v2","state_key":"","content":{},"content":{}}"#
                .into(),
        )
        .unwrap();
        let selected =
            select_approval_marker_raw_state([&raw], super::super::approval::Namespace::DEFAULT);
        assert!(selected.observed_v2);
        assert!(selected.marker.is_err());
    }
}
