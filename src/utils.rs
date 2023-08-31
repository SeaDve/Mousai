use gtk::{
    gio,
    glib::{self, prelude::*},
};

use std::{collections::BTreeSet, future::Future, ops::Range};

use crate::Application;

/// Spawns a future in the default [`glib::MainContext`]
pub fn spawn<R, F>(priority: glib::Priority, fut: F) -> glib::JoinHandle<R>
where
    R: 'static,
    F: Future<Output = R> + 'static,
{
    let ctx = glib::MainContext::default();
    ctx.spawn_local_with_priority(priority, fut)
}

/// Get the global instance of `Application`.
///
/// # Panics
/// Panics if the application is not running or if this is
/// called on a non-main thread.
pub fn app_instance() -> Application {
    debug_assert!(
        gtk::is_initialized_main_thread(),
        "application must only be accessed in the main thread"
    );

    gio::Application::default().unwrap().downcast().unwrap()
}

/// Returns a sorted list of ranges of consecutive numbers in the given set.
pub fn consecutive_groups(ordered_set: &BTreeSet<usize>) -> Vec<Range<usize>> {
    let mut iter = ordered_set.iter();

    let first = match iter.next() {
        Some(first) => *first,
        None => return Vec::new(),
    };

    // If all numbers are consecutive, return a single group
    if ordered_set.last().unwrap() - first + 1 == ordered_set.len() {
        return vec![first..first + ordered_set.len()];
    }

    let mut ret: Vec<Range<usize>> = Vec::new();

    let mut current = Range {
        start: first,
        end: first + 1,
    };

    for &num in iter {
        if num == current.end {
            // Consecutive number, increment end
            current.end += 1;
        } else {
            // Non-consecutive number, store group in result and start new group
            ret.push(current.clone());
            current.start = num;
            current.end = num + 1;
        }
    }

    // Store the last group
    ret.push(current);

    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_groups_empty() {
        assert_eq!(
            consecutive_groups(&BTreeSet::new()),
            Vec::<Range<usize>>::new()
        );
    }

    #[test]
    fn consecutive_groups_single() {
        assert_eq!(consecutive_groups(&BTreeSet::from([3])), vec![3..4]);
        assert_eq!(consecutive_groups(&BTreeSet::from([3, 3])), vec![3..4]);
        assert_eq!(consecutive_groups(&BTreeSet::from([3, 3, 3])), vec![3..4]);

        assert_eq!(consecutive_groups(&BTreeSet::from([0])), vec![0..1]);
    }

    #[test]
    fn consecutive_groups_two() {
        assert_eq!(consecutive_groups(&BTreeSet::from([1, 2])), vec![1..3]);
        assert_eq!(consecutive_groups(&BTreeSet::from([2, 2, 1])), vec![1..3]);
        assert_eq!(consecutive_groups(&BTreeSet::from([1, 2, 1])), vec![1..3]);
        assert_eq!(
            consecutive_groups(&BTreeSet::from([2, 1, 2, 1])),
            vec![1..3]
        );

        assert_eq!(consecutive_groups(&BTreeSet::from([5, 6])), vec![5..7]);
    }

    #[test]
    fn consecutive_groups_many() {
        assert_eq!(
            consecutive_groups(&BTreeSet::from([1, 2, 3, 4, 5])),
            vec![1..6]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 4, 3, 2, 1])),
            vec![1..6]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 3, 4, 2, 1])),
            vec![1..6]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 3, 4, 4, 3, 4, 2, 1])),
            vec![1..6]
        );

        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 6, 7, 8, 9])),
            vec![5..10]
        );
    }

    #[test]
    fn consecutive_groups_many_non_consecutives() {
        assert_eq!(
            consecutive_groups(&BTreeSet::from([1, 2, 3, 5, 6, 10, 12])),
            vec![1..4, 5..7, 10..11, 12..13]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([12, 1, 3, 2, 3, 6, 5, 6, 3, 10, 12, 12])),
            vec![1..4, 5..7, 10..11, 12..13]
        );

        assert_eq!(
            consecutive_groups(&BTreeSet::from([7, 8, 9, 11, 12, 14, 16])),
            vec![7..10, 11..13, 14..15, 16..17]
        );
    }

    #[test]
    fn consecutive_groups_many_non_consecutives_large_num() {
        assert_eq!(
            consecutive_groups(&BTreeSet::from([
                100_000_001,
                100_000_002,
                100_000_003,
                100_000_005,
                100_000_006,
                100_000_010,
                100_000_012
            ])),
            vec![
                100_000_001..100_000_004,
                100_000_005..100_000_007,
                100_000_010..100_000_011,
                100_000_012..100_000_013
            ]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([
                100_000_012,
                100_000_001,
                100_000_003,
                100_000_002,
                100_000_003,
                100_000_006,
                100_000_005,
                100_000_006,
                100_000_003,
                100_000_010,
                100_000_012,
                100_000_012
            ])),
            vec![
                100_000_001..100_000_004,
                100_000_005..100_000_007,
                100_000_010..100_000_011,
                100_000_012..100_000_013
            ]
        );
    }
}
