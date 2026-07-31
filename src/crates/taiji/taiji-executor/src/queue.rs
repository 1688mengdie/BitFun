//! Signal queue — FIFO + priority-ordered consumption.
//!
//! Uses a BinaryHeap internally: signals are ordered by priority first,
//! then by insertion order (FIFO) within the same priority level.
//!
//! Design reference: R-2-504 信号队列管理
//! 参考: 量价时空/Phase-2-派发提示词.md:843 — R-2-504 — taiji-executor 执行器

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::signal::{ExecSignal, SignalPriority};

/// A signal wrapped with priority + sequence metadata for heap ordering.
#[derive(Debug, Clone)]
pub(crate) struct PrioritySignal {
    pub signal: ExecSignal,
    /// Numeric priority (0 = highest).
    priority: u8,
    /// Monotonic insertion sequence — ensures FIFO within same priority.
    seq: u64,
}

impl Ord for PrioritySignal {
    /// Max-heap ordering:
    /// 1. Lower priority value (higher urgency) → Greater
    /// 2. Lower seq (earlier insertion) → Greater
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for PrioritySignal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PrioritySignal {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}

impl Eq for PrioritySignal {}

/// A thread-safe signal queue with priority-aware ordering.
///
/// # Ordering semantics
///
/// - Higher priority signals are always dequeued before lower priority ones.
/// - Within the **same priority level**, signals are consumed in FIFO order.
///
/// # Example
///
/// ```ignore
/// let mut queue = SignalQueue::new(1024);
/// queue.enqueue(signal_a);              // Normal priority
/// queue.enqueue_with_priority(signal_b, SignalPriority::High);
///
/// let next = queue.dequeue();           // Returns signal_b (High before Normal)
/// ```
pub struct SignalQueue {
    heap: BinaryHeap<PrioritySignal>,
    seq_counter: AtomicU64,
    capacity: usize,
}

impl SignalQueue {
    /// Create a new signal queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
            seq_counter: AtomicU64::new(1),
            capacity,
        }
    }

    /// Enqueue a signal with its embedded priority.
    pub fn enqueue(&mut self, signal: ExecSignal) -> Result<(), ExecSignal> {
        if self.heap.len() >= self.capacity {
            return Err(signal);
        }
        let priority = signal.priority.rank();
        let seq = self.seq_counter.fetch_add(1, AtomicOrdering::Relaxed);
        self.heap.push(PrioritySignal { signal, priority, seq });
        Ok(())
    }

    /// Enqueue with an explicit priority override.
    pub fn enqueue_with_priority(
        &mut self,
        signal: ExecSignal,
        priority: SignalPriority,
    ) -> Result<(), ExecSignal> {
        let mut s = signal;
        s.priority = priority;
        self.enqueue(s)
    }

    /// Dequeue the highest-priority signal (FIFO within same priority).
    pub fn dequeue(&mut self) -> Option<ExecSignal> {
        self.heap.pop().map(|ps| ps.signal)
    }

    /// Dequeue up to `max_count` signals in priority order.
    pub fn dequeue_batch(&mut self, max_count: usize) -> Vec<ExecSignal> {
        let mut result = Vec::with_capacity(max_count.min(self.heap.len()));
        for _ in 0..max_count {
            match self.heap.pop() {
                Some(ps) => result.push(ps.signal),
                None => break,
            }
        }
        result
    }

    /// Peek at the next signal without removing it.
    pub fn peek(&self) -> Option<&ExecSignal> {
        self.heap.peek().map(|ps| &ps.signal)
    }

    /// Return the number of queued signals.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Return true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Drain all signals in priority order.
    pub fn drain(&mut self) -> Vec<ExecSignal> {
        let mut result = Vec::with_capacity(self.heap.len());
        while let Some(ps) = self.heap.pop() {
            result.push(ps.signal);
        }
        result
    }

    /// Clear all signals from the queue.
    pub fn clear(&mut self) {
        self.heap.clear();
    }

    /// Return the maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for SignalQueue {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{ExecAction, ExecSignal};

    fn make_signal(instrument: &str, confidence: f64) -> ExecSignal {
        ExecSignal::new(instrument, ExecAction::OpenLong, Some(100.0), 1.0, "test", confidence)
    }

    #[test]
    fn test_fifo_within_same_priority() {
        let mut queue = SignalQueue::new(128);
        let s1 = make_signal("ag2506", 0.8);
        let s2 = make_signal("rb2501", 0.7);
        let s3 = make_signal("cu2507", 0.9);

        queue.enqueue(s1.clone()).unwrap();
        queue.enqueue(s2.clone()).unwrap();
        queue.enqueue(s3.clone()).unwrap();

        // All Normal priority → FIFO order.
        assert_eq!(queue.dequeue().unwrap().id, s1.id);
        assert_eq!(queue.dequeue().unwrap().id, s2.id);
        assert_eq!(queue.dequeue().unwrap().id, s3.id);
    }

    #[test]
    fn test_priority_before_fifo() {
        let mut queue = SignalQueue::new(128);
        let normal = make_signal("ag2506", 0.8);
        let high = make_signal("rb2501", 1.0).with_priority(SignalPriority::High);
        let low = make_signal("cu2507", 0.5).with_priority(SignalPriority::Low);

        queue.enqueue(normal.clone()).unwrap();
        queue.enqueue(high.clone()).unwrap();
        queue.enqueue(low.clone()).unwrap();

        // High first, then Normal, then Low.
        assert_eq!(queue.dequeue().unwrap().id, high.id);
        assert_eq!(queue.dequeue().unwrap().id, normal.id);
        assert_eq!(queue.dequeue().unwrap().id, low.id);
    }

    #[test]
    fn test_capacity_limit() {
        let mut queue = SignalQueue::new(2);
        let s1 = make_signal("ag2506", 0.8);
        let s2 = make_signal("rb2501", 0.7);
        let s3 = make_signal("cu2507", 0.9);

        assert!(queue.enqueue(s1).is_ok());
        assert!(queue.enqueue(s2).is_ok());
        assert!(queue.enqueue(s3).is_err()); // capacity exceeded
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_dequeue_batch() {
        let mut queue = SignalQueue::new(128);
        queue.enqueue(make_signal("a", 0.8)).unwrap();
        queue.enqueue(make_signal("b", 0.7)).unwrap();
        queue.enqueue(make_signal("c", 0.9)).unwrap();

        let batch = queue.dequeue_batch(2);
        assert_eq!(batch.len(), 2);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_peek() {
        let mut queue = SignalQueue::new(128);
        let s1 = make_signal("ag2506", 0.8);
        queue.enqueue(s1.clone()).unwrap();

        let peeked = queue.peek().unwrap();
        assert_eq!(peeked.id, s1.id);
        assert_eq!(queue.len(), 1); // peek does not remove
    }

    #[test]
    fn test_drain() {
        let mut queue = SignalQueue::new(128);
        queue.enqueue(make_signal("a", 0.8)).unwrap();
        queue.enqueue(make_signal("b", 0.7)).unwrap();

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut queue = SignalQueue::new(128);
        queue.enqueue(make_signal("a", 0.8)).unwrap();
        queue.enqueue(make_signal("b", 0.7)).unwrap();
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_empty_queue() {
        let mut queue = SignalQueue::new(128);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.dequeue().is_none());
        assert!(queue.peek().is_none());
    }

    #[test]
    fn test_enqueue_with_priority() {
        let mut queue = SignalQueue::new(128);
        let sig = make_signal("ag2506", 0.8);
        queue.enqueue_with_priority(sig, SignalPriority::High).unwrap();
        assert_eq!(queue.len(), 1);
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.priority, SignalPriority::High);
    }
}
