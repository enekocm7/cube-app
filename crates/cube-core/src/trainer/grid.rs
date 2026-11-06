//! Moving through the case picker from the keyboard.
//!
//! The picker lays cases out in groups of rows, so its arrows need to know
//! where a row ends and where the next group starts.

/// A direction in the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridStep {
    Left,
    Right,
    Up,
    Down,
}

/// The group holding `cursor`, and that group's first position.
fn locate(groups: &[usize], cursor: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for (index, &size) in groups.iter().enumerate() {
        if cursor < start + size {
            return Some((index, start));
        }
        start += size;
    }
    None
}

/// Returns where a step from `cursor` lands, in a grid of `columns` columns
/// whose groups have the sizes in `groups`. Each group starts a new row, and
/// positions count through the groups in order.
///
/// Going past an end stays put. Going up or down keeps the column, as far as
/// the row it lands on has one.
pub fn grid_step(groups: &[usize], columns: usize, cursor: usize, step: GridStep) -> usize {
    let total: usize = groups.iter().sum();
    if total == 0 {
        return 0;
    }
    let cursor = cursor.min(total - 1);
    let columns = columns.max(1);
    let Some((group, start)) = locate(groups, cursor) else {
        return cursor;
    };
    let size = groups[group];
    let place = cursor - start;
    let column = place % columns;
    match step {
        GridStep::Left => cursor.saturating_sub(1),
        GridStep::Right => (cursor + 1).min(total - 1),
        GridStep::Down if place / columns < (size - 1) / columns => {
            start + (place + columns).min(size - 1)
        }
        GridStep::Up if place >= columns => cursor - columns,
        GridStep::Down => (group + 1..groups.len())
            .find(|&next| groups[next] > 0)
            .map_or(cursor, |next| {
                let first: usize = groups[..next].iter().sum();
                first + column.min(groups[next] - 1)
            }),
        GridStep::Up => (0..group)
            .rev()
            .find(|&previous| groups[previous] > 0)
            .map_or(cursor, |previous| {
                let first: usize = groups[..previous].iter().sum();
                let size = groups[previous];
                let last_row = (size - 1) / columns * columns;
                first + (last_row + column).min(size - 1)
            }),
    }
}

/// Returns how many rows of cases and how many group headings come before and
/// including the row `cursor` is on, not counting that row itself.
pub fn rows_before(groups: &[usize], columns: usize, cursor: usize) -> (usize, usize) {
    let columns = columns.max(1);
    let mut rows = 0;
    let mut start = 0;
    for (index, &size) in groups.iter().enumerate() {
        if cursor < start + size {
            return (rows + (cursor - start) / columns, index + 1);
        }
        rows += size.div_ceil(columns);
        start += size;
    }
    (rows.saturating_sub(1), groups.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Groups of five and two, three columns:
    //   0 1 2
    //   3 4
    //   5 6
    const GROUPS: [usize; 2] = [5, 2];

    #[test]
    fn left_and_right_walk_every_position() {
        assert_eq!(grid_step(&GROUPS, 3, 2, GridStep::Right), 3);
        assert_eq!(grid_step(&GROUPS, 3, 3, GridStep::Left), 2);
        assert_eq!(grid_step(&GROUPS, 3, 4, GridStep::Right), 5);
    }

    #[test]
    fn the_ends_stay_put() {
        assert_eq!(grid_step(&GROUPS, 3, 0, GridStep::Left), 0);
        assert_eq!(grid_step(&GROUPS, 3, 6, GridStep::Right), 6);
        assert_eq!(grid_step(&GROUPS, 3, 0, GridStep::Up), 0);
        assert_eq!(grid_step(&GROUPS, 3, 6, GridStep::Down), 6);
    }

    #[test]
    fn down_keeps_the_column_within_a_group() {
        assert_eq!(grid_step(&GROUPS, 3, 1, GridStep::Down), 4);
    }

    #[test]
    fn down_lands_on_a_short_row_at_its_end() {
        assert_eq!(grid_step(&GROUPS, 3, 2, GridStep::Down), 4);
    }

    #[test]
    fn down_crosses_into_the_next_group() {
        assert_eq!(grid_step(&GROUPS, 3, 3, GridStep::Down), 5);
        assert_eq!(grid_step(&GROUPS, 3, 4, GridStep::Down), 6);
    }

    #[test]
    fn up_crosses_into_the_last_row_of_the_previous_group() {
        assert_eq!(grid_step(&GROUPS, 3, 5, GridStep::Up), 3);
        assert_eq!(grid_step(&GROUPS, 3, 6, GridStep::Up), 4);
    }

    #[test]
    fn up_keeps_the_column_within_a_group() {
        assert_eq!(grid_step(&GROUPS, 3, 4, GridStep::Up), 1);
    }

    #[test]
    fn a_cursor_past_the_end_comes_back() {
        assert_eq!(grid_step(&GROUPS, 3, 99, GridStep::Left), 5);
    }

    #[test]
    fn nothing_to_move_through_stays_at_zero() {
        assert_eq!(grid_step(&[], 3, 0, GridStep::Down), 0);
    }

    #[test]
    fn rows_and_headings_count_through_the_groups() {
        assert_eq!(rows_before(&GROUPS, 3, 0), (0, 1));
        assert_eq!(rows_before(&GROUPS, 3, 4), (1, 1));
        assert_eq!(rows_before(&GROUPS, 3, 6), (2, 2));
    }
}
