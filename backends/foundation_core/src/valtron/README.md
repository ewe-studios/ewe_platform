# Valtron
An experimental async runtime built on iterators that do not specifically rely on the async future paradigm but instead on the potential of processes being represented as specialized and generic iterators that work both in async and sync contexts.

The sub-crate provides useful ideas on how asynchronouse operations can be organized supported by a lightweight thread based runtime that builds on a very simple abstraction.

```rust
let now = Instant::now();
let wait = Duration::from_secs(3);
let mut sleeper = SleepIterator::new(now, wait, ());

assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

thread::sleep(Duration::from_secs(1));

assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

thread::sleep(Duration::from_secs(2));

assert_eq!(sleeper.next(), Some(Delayed::Done(())));
```
