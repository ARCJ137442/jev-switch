//! 事件日志系统（Dashboard 实时流水数据源）。
//!
//! 简化实现：内存环形缓冲区 + polling API（GET /v1/admin/logs?since=N）。
//! SSE 推送可后续扩展，当前优先完成 Dashboard 展示。

use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// 单条事件记录
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct Event {
    /// 自增序号（用于 polling since 参数）
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub id: u64,
    /// Unix 时间戳（毫秒）
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub timestamp: u64,
    /// 事件类型：request | probe | config_change | error
    pub kind: String,
    /// 事件详情
    pub detail: String,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub token_id: Option<String>,
}

/// 事件总线（环形缓冲区，最多保留 N 条）
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Mutex<EventBusInner>>,
}

struct EventBusInner {
    events: Vec<Event>,
    next_id: u64,
    capacity: usize,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventBusInner {
                events: Vec::with_capacity(capacity),
                next_id: 1,
                capacity,
            })),
        }
    }

    /// 推送新事件
    pub fn push(&self, kind: impl Into<String>, detail: impl Into<String>) {
        self.push_for_token(kind, detail, None);
    }

    pub fn push_for_token(
        &self,
        kind: impl Into<String>,
        detail: impl Into<String>,
        token_id: Option<String>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;

        Self::push_locked(&mut inner, id, kind.into(), detail.into(), token_id);
    }

    /// Publish a durable event using the owning call-log row's ID. The admin
    /// polling API reads the same IDs from SQLite, so SSE and refresh cursors
    /// remain aligned across daemon restarts.
    pub fn push_for_token_with_id(
        &self,
        id: u64,
        kind: impl Into<String>,
        detail: impl Into<String>,
        token_id: Option<String>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.next_id = inner.next_id.max(id.saturating_add(1));
        inner.events.retain(|event| event.id != id);
        Self::push_locked(&mut inner, id, kind.into(), detail.into(), token_id);
    }

    fn push_locked(
        inner: &mut EventBusInner,
        id: u64,
        kind: String,
        detail: String,
        token_id: Option<String>,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let event = Event {
            id,
            timestamp,
            kind,
            detail,
            token_id,
        };

        inner.events.push(event);
        inner.events.sort_by_key(|event| event.id);

        // 环形缓冲：超出容量时移除最旧的
        if inner.events.len() > inner.capacity {
            inner.events.remove(0);
        }
    }

    /// 获取从 since_id 之后的所有事件（不含 since_id）
    pub fn since(&self, since_id: u64) -> Vec<Event> {
        let inner = self.inner.lock().unwrap();
        inner
            .events
            .iter()
            .filter(|e| e.id > since_id)
            .cloned()
            .collect()
    }

    pub fn page(&self, since_id: u64, limit: usize, token_id: Option<&str>) -> Vec<Event> {
        let inner = self.inner.lock().unwrap();
        inner
            .events
            .iter()
            .filter(|event| {
                event.id > since_id
                    && token_id
                        .map(|id| event.token_id.as_deref() == Some(id))
                        .unwrap_or(true)
            })
            .take(limit.clamp(1, 500))
            .cloned()
            .collect()
    }

    /// 获取最近 N 条事件
    pub fn recent(&self, limit: usize) -> Vec<Event> {
        let inner = self.inner.lock().unwrap();
        let len = inner.events.len();
        if len <= limit {
            inner.events.clone()
        } else {
            inner.events[len - limit..].to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_evicts_oldest() {
        let bus = EventBus::new(3);
        bus.push("test", "event1");
        bus.push("test", "event2");
        bus.push("test", "event3");
        bus.push("test", "event4");

        let recent = bus.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].detail, "event2");
        assert_eq!(recent[2].detail, "event4");
    }

    #[test]
    fn since_filters_correctly() {
        let bus = EventBus::new(10);
        bus.push("test", "event1");
        bus.push("test", "event2");
        let events = bus.recent(10);
        let id1 = events[0].id;

        bus.push("test", "event3");
        let new_events = bus.since(id1);
        assert_eq!(new_events.len(), 2);
        assert_eq!(new_events[0].detail, "event2");
        assert_eq!(new_events[1].detail, "event3");
    }

    #[test]
    fn durable_ids_keep_event_order_and_advance_the_live_cursor() {
        let bus = EventBus::new(10);
        bus.push_for_token_with_id(41, "request", "older", None);
        bus.push_for_token_with_id(43, "request", "newer", None);
        bus.push_for_token_with_id(42, "request", "middle", None);

        let events = bus.since(41);
        assert_eq!(
            events.iter().map(|event| event.id).collect::<Vec<_>>(),
            vec![42, 43]
        );

        bus.push("request", "next");
        assert_eq!(bus.recent(1)[0].id, 44);
    }
}
