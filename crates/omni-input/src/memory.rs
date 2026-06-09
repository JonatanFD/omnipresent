//! In-memory adapters for the input ports. They need no OS and cannot fail, so
//! they are ideal for unit tests and for driving the pipelines without real
//! hardware. The real per-OS adapters will live alongside these.

use crate::port::{InputSink, InputSource};
use omni_protocol::InputEvent;
use std::collections::VecDeque;
use std::convert::Infallible;

/// An [`InputSource`] that replays a fixed script of events in order, then
/// reports `None`. Stands in for real capture in tests.
#[derive(Debug, Default)]
pub struct QueuedSource {
    queue: VecDeque<InputEvent>,
}

impl QueuedSource {
    /// An empty source that will immediately report `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// A source preloaded with events to replay, in iteration order.
    pub fn from_events<I: IntoIterator<Item = InputEvent>>(events: I) -> Self {
        Self {
            queue: events.into_iter().collect(),
        }
    }

    /// Queues another event to be returned by a later `poll`.
    pub fn push(&mut self, event: InputEvent) {
        self.queue.push_back(event);
    }

    /// How many events are still queued.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether no events remain.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl InputSource for QueuedSource {
    type Error = Infallible;

    fn poll(&mut self) -> Result<Option<InputEvent>, Self::Error> {
        Ok(self.queue.pop_front())
    }
}

/// An [`InputSink`] that records every event injected, in order. Stands in for
/// real injection in tests.
#[derive(Debug, Default)]
pub struct RecordingSink {
    injected: Vec<InputEvent>,
}

impl RecordingSink {
    /// A sink that has recorded nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The events injected so far, in order.
    pub fn injected(&self) -> &[InputEvent] {
        &self.injected
    }
}

impl InputSink for RecordingSink {
    type Error = Infallible;

    fn inject(&mut self, event: InputEvent) -> Result<(), Self::Error> {
        self.injected.push(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omni_protocol::input::{Action, KeyCode, Modifiers, MouseButton, MouseDelta, ScrollDelta};

    fn sample_events() -> Vec<InputEvent> {
        vec![
            InputEvent::Motion(MouseDelta::new(3, -2)),
            InputEvent::Key {
                code: KeyCode::new(0x04),
                action: Action::Press,
                modifiers: Modifiers::SHIFT,
            },
            InputEvent::Button {
                button: MouseButton::Left,
                action: Action::Release,
            },
            InputEvent::Scroll(ScrollDelta::new(0, 1)),
        ]
    }

    #[test]
    fn empty_source_reports_none() {
        let mut source = QueuedSource::new();
        assert!(source.is_empty());
        assert_eq!(source.poll().unwrap(), None);
    }

    #[test]
    fn source_replays_events_in_order_then_none() {
        let events = sample_events();
        let mut source = QueuedSource::from_events(events.clone());

        for expected in &events {
            assert_eq!(source.poll().unwrap().as_ref(), Some(expected));
        }
        assert_eq!(source.poll().unwrap(), None);
        assert!(source.is_empty());
    }

    #[test]
    fn pushed_events_are_polled_back() {
        let mut source = QueuedSource::new();
        let event = InputEvent::Motion(MouseDelta::new(1, 1));
        source.push(event);

        assert_eq!(source.len(), 1);
        assert_eq!(source.poll().unwrap(), Some(event));
    }

    #[test]
    fn sink_records_injected_events_in_order() {
        let mut sink = RecordingSink::new();
        let events = sample_events();

        for &event in &events {
            sink.inject(event).unwrap();
        }

        assert_eq!(sink.injected(), events.as_slice());
    }

    #[test]
    fn source_and_sink_compose_into_a_pipeline() {
        // Pump every captured event straight into the sink — the shape of the
        // Runtime's receive -> inject loop.
        let events = sample_events();
        let mut source = QueuedSource::from_events(events.clone());
        let mut sink = RecordingSink::new();

        while let Some(event) = source.poll().unwrap() {
            sink.inject(event).unwrap();
        }

        assert_eq!(sink.injected(), events.as_slice());
    }
}
