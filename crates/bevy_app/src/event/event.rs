use legion::prelude::{Resources, Schedulable, SystemBuilder};
use std::marker::PhantomData;

struct EventInstance<T> {
    pub event_count: usize,
    pub event: T,
}

enum State {
    A,
    B,
}

/// An event collection that represents the events that occurred within the last two [Events::update] calls. Events can be cheaply read using
/// an [EventReader]. This collection is meant to be paired with a system that calls [Events::update] exactly once per update/frame. [Events::build_update_system]
/// will produce a system that does this. [EventReader]s are expected to read events from this collection at least once per update/frame. If events are not handled
/// within one frame/update, they will be dropped.
///
/// # Example
/// ```
/// use bevy_app::Events;
///
/// struct MyEvent {
///     value: usize
/// }
///
/// // setup
/// let mut events = Events::<MyEvent>::default();
/// let mut reader = events.get_reader();
///
/// // run this once per update/frame
/// events.update();
///
/// // somewhere else: send an event
/// events.send(MyEvent { value: 1 });
///
/// // somewhere else: read the events
/// for event in reader.iter(&events) {
///     assert_eq!(event.value, 1)
/// }
///
/// // events are only processed once per reader
/// assert_eq!(reader.iter(&events).count(), 0);
/// ```  
///
/// # Details
///
/// [Events] is implemented using a double buffer. Each call to [Events::update] swaps buffers and clears out the oldest buffer.
/// [EventReader]s that read at least once per update will never drop events. [EventReader]s that read once within two updates might
/// still receive some events. [EventReader]s that read after two updates are guaranteed to drop all events that occurred before those updates.
///
/// The buffers in [Events] will grow indefinitely if [Events::update] is never called.
///
/// An alternative call pattern would be to call [Events::update] manually across frames to control when events are cleared. However
/// this complicates consumption
pub struct Events<T>
where
    T: Send + Sync + 'static,
{
    events_a: Vec<EventInstance<T>>,
    events_b: Vec<EventInstance<T>>,
    a_start_event_count: usize,
    b_start_event_count: usize,
    event_count: usize,
    state: State,
}

impl<T> Default for Events<T>
where
    T: Send + Sync + 'static,
{
    fn default() -> Self {
        Events {
            a_start_event_count: 0,
            b_start_event_count: 0,
            event_count: 0,
            events_a: Vec::new(),
            events_b: Vec::new(),
            state: State::A,
        }
    }
}

fn map_event_instance<T>(event_instance: &EventInstance<T>) -> &T
where
    T: Send + Sync + 'static,
{
    &event_instance.event
}

pub struct EventReader<T> {
    last_event_count: usize,
    _marker: PhantomData<T>,
}

impl<T> EventReader<T>
where
    T: Send + Sync + 'static,
{
    /// Iterates over the events this EventReader has not seen yet. This updates the EventReader's
    /// event counter, which means subsequent event reads will not include events that happened before now.
    pub fn iter<'a>(&mut self, events: &'a Events<T>) -> impl DoubleEndedIterator<Item = &'a T> {
        // if the reader has seen some of the events in a buffer, find the proper index offset.
        // otherwise read all events in the buffer
        let a_index = if self.last_event_count > events.a_start_event_count {
            self.last_event_count - events.a_start_event_count
        } else {
            0
        };
        let b_index = if self.last_event_count > events.b_start_event_count {
            self.last_event_count - events.b_start_event_count
        } else {
            0
        };
        self.last_event_count = events.event_count;
        match events.state {
            State::A => events
                .events_b
                .get(b_index..)
                .unwrap_or_else(|| &[])
                .iter()
                .map(map_event_instance)
                .chain(
                    events
                        .events_a
                        .get(a_index..)
                        .unwrap_or_else(|| &[])
                        .iter()
                        .map(map_event_instance),
                ),
            State::B => events
                .events_a
                .get(a_index..)
                .unwrap_or_else(|| &[])
                .iter()
                .map(map_event_instance)
                .chain(
                    events
                        .events_b
                        .get(b_index..)
                        .unwrap_or_else(|| &[])
                        .iter()
                        .map(map_event_instance),
                ),
        }
    }

    /// Retrieves the latest event that this EventReader hasn't seen yet. This updates the EventReader's
    /// event counter, which means subsequent event reads will not include events that happened before now.
    pub fn latest<'a>(&mut self, events: &'a Events<T>) -> Option<&'a T> {
        self.iter(events).rev().next()
    }

    /// Retrieves the latest event that matches the given `predicate` that this reader hasn't seen yet. This updates the EventReader's
    /// event counter, which means subsequent event reads will not include events that happened before now.
    pub fn find_latest<'a>(
        &mut self,
        events: &'a Events<T>,
        predicate: impl FnMut(&&T) -> bool,
    ) -> Option<&'a T> {
        self.iter(events).rev().filter(predicate).next()
    }

    /// Retrieves the earliest event in `events` that this reader hasn't seen yet. This updates the EventReader's
    /// event counter, which means subsequent event reads will not include events that happened before now.
    pub fn earliest<'a>(&mut self, events: &'a Events<T>) -> Option<&'a T> {
        self.iter(events).next()
    }
}

impl<T> Events<T>
where
    T: Send + Sync + 'static,
{
    /// "Sends" an `event` by writing it to the current event buffer. [EventReader]s can then read the event.
    pub fn send(&mut self, event: T) {
        let event_instance = EventInstance {
            event,
            event_count: self.event_count,
        };

        match self.state {
            State::A => self.events_a.push(event_instance),
            State::B => self.events_b.push(event_instance),
        }

        self.event_count += 1;
    }

    /// Gets a new [EventReader]. This will include all events already in the event buffers.
    pub fn get_reader(&self) -> EventReader<T> {
        EventReader {
            last_event_count: 0,
            _marker: PhantomData,
        }
    }

    /// Gets a new [EventReader]. This will ignore all events already in the event buffers. It will read all future events.
    pub fn get_reader_current(&self) -> EventReader<T> {
        EventReader {
            last_event_count: self.event_count,
            _marker: PhantomData,
        }
    }

    /// Swaps the event buffers and clears the oldest event buffer. In general, this should be called once per frame/update.
    pub fn update(&mut self) {
        match self.state {
            State::A => {
                self.events_b = Vec::new();
                self.state = State::B;
                self.b_start_event_count = self.event_count;
            }
            State::B => {
                self.events_a = Vec::new();
                self.state = State::A;
                self.a_start_event_count = self.event_count;
            }
        }
    }

    /// Builds a system that calls [Events::update] once per frame.
    pub fn build_update_system() -> Box<dyn Schedulable> {
        SystemBuilder::new(format!("events_update::{}", std::any::type_name::<T>()))
            .write_resource::<Self>()
            .build(|_, _, events, _| events.update())
    }
}

pub trait GetEventReader {
    /// returns an [EventReader] of the given type
    fn get_event_reader<T>(&self) -> EventReader<T>
    where
        T: Send + Sync + 'static;
}

impl GetEventReader for Resources {
    fn get_event_reader<T>(&self) -> EventReader<T>
    where
        T: Send + Sync + 'static,
    {
        let my_event = self
            .get::<Events<T>>()
            .unwrap_or_else(|| panic!("Event does not exist: {}", std::any::type_name::<T>()));
        my_event.get_reader()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    struct TestEvent {
        i: usize,
    }

    #[test]
    fn test_events() {
        let mut events = Events::<TestEvent>::default();
        let event_0 = TestEvent { i: 0 };
        let event_1 = TestEvent { i: 1 };
        let event_2 = TestEvent { i: 2 };

        // this reader will miss event_0 and event_1 because it wont read them over the course of two updates
        let mut reader_missed = events.get_reader();

        let mut reader_a = events.get_reader();

        events.send(event_0);

        assert_eq!(
            get_events(&events, &mut reader_a),
            vec![event_0],
            "reader_a created before event receives event"
        );
        assert_eq!(
            get_events(&events, &mut reader_a),
            vec![],
            "second iteration of reader_a created before event results in zero events"
        );

        let mut reader_b = events.get_reader();

        assert_eq!(
            get_events(&events, &mut reader_b),
            vec![event_0],
            "reader_b created after event receives event"
        );
        assert_eq!(
            get_events(&events, &mut reader_b),
            vec![],
            "second iteration of reader_b created after event results in zero events"
        );

        events.send(event_1);

        let mut reader_c = events.get_reader();

        assert_eq!(
            get_events(&events, &mut reader_c),
            vec![event_0, event_1],
            "reader_c created after two events receives both events"
        );
        assert_eq!(
            get_events(&events, &mut reader_c),
            vec![],
            "second iteration of reader_c created after two event results in zero events"
        );

        assert_eq!(
            get_events(&events, &mut reader_a),
            vec![event_1],
            "reader_a receives next unread event"
        );

        events.update();

        let mut reader_d = events.get_reader();

        events.send(event_2);

        assert_eq!(
            get_events(&events, &mut reader_a),
            vec![event_2],
            "reader_a receives event created after update"
        );
        assert_eq!(
            get_events(&events, &mut reader_b),
            vec![event_1, event_2],
            "reader_b receives events created before and after update"
        );
        assert_eq!(
            get_events(&events, &mut reader_d),
            vec![event_0, event_1, event_2],
            "reader_d receives all events created before and after update"
        );

        events.update();

        assert_eq!(
            get_events(&events, &mut reader_missed),
            vec![event_2],
            "reader_missed missed events unread after to update() calls"
        );
    }

    fn get_events(
        events: &Events<TestEvent>,
        reader: &mut EventReader<TestEvent>,
    ) -> Vec<TestEvent> {
        reader.iter(events).cloned().collect::<Vec<TestEvent>>()
    }
}