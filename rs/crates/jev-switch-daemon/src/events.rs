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
    pub id: u64,
    /// Unix 时间戳（毫秒）
    pub timestamp: u64,
    /// 事件类型：request | probe | config_change | error
    pub kind: String,
    /// 事件详情
    pub detail: String,
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
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let event = Event {
            id,
            timestamp,
            kind: kind.into(),
            detail: detail.into(),
        };

        inner.events.push(event);

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
}
