use std::{collections::BTreeSet, future::Future};

use gtk::glib;

/// Spawns a future in the default [`glib::MainContext`]
pub fn spawn<R, F>(priority: glib::Priority, fut: F) -> glib::JoinHandle<R>
where
    R: 'static,
    F: Future<Output = R> + 'static,
{
    let ctx = glib::MainContext::default();
    ctx.spawn_local_with_priority(priority, fut)
}

/// Returns a list of tuples where the first element of a tuple is the first number
/// in a consecutive group, and the second element is the count of numbers in that group.
pub fn consecutive_groups(ordered_set: &BTreeSet<usize>) -> Vec<(usize, usize)> {
    let mut iter = ordered_set.iter();

    let first = match iter.next() {
        Some(first) => *first,
        None => return Vec::new(),
    };

    // If all numbers are consecutive, return a single group
    if ordered_set.last().unwrap() - first + 1 == ordered_set.len() {
        return vec![(first, ordered_set.len())];
    }

    let mut ret: Vec<(usize, usize)> = Vec::new();

    let mut current_group_start = first;
    let mut current_group_count = 1;

    for &num in iter {
        if num == current_group_start + current_group_count {
            // Consecutive number, increment count
            current_group_count += 1;
        } else {
            // Non-consecutive number, store group in result and start new group
            ret.push((current_group_start, current_group_count));
            current_group_start = num;
            current_group_count = 1;
        }
    }

    // Store the last group
    ret.push((current_group_start, current_group_count));

    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_groups_empty() {
        assert_eq!(consecutive_groups(&BTreeSet::new()), vec![]);
    }

    #[test]
    fn consecutive_groups_single() {
        assert_eq!(consecutive_groups(&BTreeSet::from([3])), vec![(3, 1)]);
        assert_eq!(consecutive_groups(&BTreeSet::from([3, 3])), vec![(3, 1)]);
        assert_eq!(consecutive_groups(&BTreeSet::from([3, 3, 3])), vec![(3, 1)]);

        assert_eq!(consecutive_groups(&BTreeSet::from([0])), vec![(0, 1)]);
    }

    #[test]
    fn consecutive_groups_two() {
        assert_eq!(consecutive_groups(&BTreeSet::from([1, 2])), vec![(1, 2)]);
        assert_eq!(consecutive_groups(&BTreeSet::from([2, 2, 1])), vec![(1, 2)]);
        assert_eq!(consecutive_groups(&BTreeSet::from([1, 2, 1])), vec![(1, 2)]);
        assert_eq!(
            consecutive_groups(&BTreeSet::from([2, 1, 2, 1])),
            vec![(1, 2)]
        );

        assert_eq!(consecutive_groups(&BTreeSet::from([5, 6])), vec![(5, 2)]);
    }

    #[test]
    fn consecutive_groups_many() {
        assert_eq!(
            consecutive_groups(&BTreeSet::from([1, 2, 3, 4, 5])),
            vec![(1, 5)]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 4, 3, 2, 1])),
            vec![(1, 5)]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 3, 4, 2, 1])),
            vec![(1, 5)]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 3, 4, 4, 3, 4, 2, 1])),
            vec![(1, 5)]
        );

        assert_eq!(
            consecutive_groups(&BTreeSet::from([5, 6, 7, 8, 9])),
            vec![(5, 5)]
        );
    }

    #[test]
    fn consecutive_groups_many_non_consecutives() {
        assert_eq!(
            consecutive_groups(&BTreeSet::from([1, 2, 3, 5, 6, 10, 12])),
            vec![(1, 3), (5, 2), (10, 1), (12, 1)]
        );
        assert_eq!(
            consecutive_groups(&BTreeSet::from([12, 1, 3, 2, 3, 6, 5, 6, 3, 10, 12, 12])),
            vec![(1, 3), (5, 2), (10, 1), (12, 1)]
        );

        assert_eq!(
            consecutive_groups(&BTreeSet::from([7, 8, 9, 11, 12, 14, 16])),
            vec![(7, 3), (11, 2), (14, 1), (16, 1)]
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
                (100_000_001, 3),
                (100_000_005, 2),
                (100_000_010, 1),
                (100_000_012, 1)
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
                (100_000_001, 3),
                (100_000_005, 2),
                (100_000_010, 1),
                (100_000_012, 1)
            ]
        );
    }
}
