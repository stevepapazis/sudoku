use std::fmt::{self, Display, Formatter};

const BLOCK_SIDE: usize = 3;
const SIDE: usize = BLOCK_SIDE * BLOCK_SIDE;
const CELL_COUNT: usize = SIDE * SIDE;
const PEER_COUNT: usize = 3 * (SIDE - 1) - 2 * (BLOCK_SIDE - 1);
/// empty cell placeholder
const EMPTY_CELL: u8 = 0;

#[inline]
const fn to_index(row: usize, col: usize) -> usize {
    SIDE * row + col
}

#[inline]
const fn to_coords(index: usize) -> (usize, usize) {
    (index / SIDE, index % SIDE)
}

#[inline]
const fn to_block(row: usize, col: usize) -> (usize, usize) {
    (row / BLOCK_SIDE, col / BLOCK_SIDE)
}

/// lookup table of cells sharing a row, column, or block
const PEERS: [[usize; PEER_COUNT]; CELL_COUNT] = {
    let mut peers = [[0usize; PEER_COUNT]; CELL_COUNT];
    let mut index = 0;

    while index < CELL_COUNT {
        let (row, col) = to_coords(index);
        let block = to_block(row, col);
        let mut count = 0;

        let mut r = 0;
        while r < SIDE {
            let mut c = 0;
            while c < SIDE {
                let b = to_block(r, c);
                if index != to_index(r, c) && (
                    (r == row) || (c == col) || ((b.0 == block.0) && (b.1 == block.1))
                ) {
                    peers[index][count] = to_index(r, c);
                    count += 1;
                }
                c += 1;
            }
            r += 1;
        }

        index += 1;
    }

    peers
};

#[derive(Debug, thiserror::Error)]
pub enum SudokuError {
    #[error("invalid number of lines; expected {SIDE} lines")]
    InvalidRowCount,

    #[error("invalid number of columns in line {line}; expected {SIDE} columns", line = .row + 1)]
    InvalidColumnCount { row: usize },

    #[error("unexpected clue in line {line}, column {column}", line = .row + 1, column = .col + 1)]
    InvalidClue { row: usize, col: usize, },

    #[error("the puzzle clues are inconsistent")]
    InconsistentPuzzle,

    #[error("the puzzle has no solutions")]
    NoSolution,
}

#[derive(Debug, Clone)]
pub struct Sudoku {
    grid: [u8; CELL_COUNT],
}

impl Sudoku {
    fn new_empty() -> Self {
        Self { grid: [EMPTY_CELL; CELL_COUNT] }
    }

    pub fn try_from_grid(grid: [u8; CELL_COUNT]) -> Result<Self, SudokuError> {
        let sudoku = Self { grid };

        for row in 0..SIDE {
            for col in 0..SIDE {
                let digit = grid[to_index(row, col)];
                if digit > SIDE as u8 {
                    return Err(SudokuError::InvalidClue { row, col });
                }
            }
        }

        if !sudoku.is_consistent() {
            return Err(SudokuError::InconsistentPuzzle);
        }

        Ok(sudoku)
    }

    /// are there conflicts between solved peers?
    pub fn is_consistent(&self) -> bool {
        (0..CELL_COUNT)
            .filter(|&index| self.grid[index] != EMPTY_CELL)
            .all(|index|
                PEERS[index]
                    .iter()
                    .all(|&peer|
                        self.grid[index] != self.grid[peer]
                    )
            )
    }

    pub fn is_filled(&self) -> bool {
        self.grid.iter().all(|&digit| digit != 0)
    }

    pub fn is_solved(&self) -> bool {
        self.is_filled() && self.is_consistent()
    }

    pub fn solve(&self) -> Result<Sudoku, SudokuError> {
        let list = CandidateList::try_from(self)?;
        if let Some(solved_list) = list.eliminate_candidates() {
            Ok(Sudoku::from(&solved_list))
        } else {
            Err(SudokuError::NoSolution)
        }
    }
}

impl Display for Sudoku {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for row in 0..SIDE {
            if f.alternate() && row > 0 && row % BLOCK_SIDE == 0 {
                writeln!(f, "------+-------+------")?;
            }

            for col in 0..SIDE {
                if f.alternate() && col > 0 {
                    if col % BLOCK_SIDE == 0 {
                        write!(f, " |")?;
                    }
                    write!(f, " ")?;
                }

                write!(f, "{}", self.grid[to_index(row, col)])?;
            }
            if row + 1 < SIDE {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl TryFrom<&str> for Sudoku {
    type Error = SudokuError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut grid = [EMPTY_CELL; CELL_COUNT];
        let mut row_count = 0;

        for (r, line) in value.lines().enumerate() {
            if r >= SIDE {
                return Err(SudokuError::InvalidRowCount);
            }

            let mut col_count = 0;

            for (c, ch) in line.chars().enumerate() {
                if c >= SIDE {
                    return Err(SudokuError::InvalidColumnCount { row: r });
                }

                let digit = match ch {
                    '0'..='9' => ch as u8 - b'0',
                    _ => {
                        return Err(SudokuError::InvalidClue {
                            row: r,
                            col: c,
                        });
                    }
                };

                grid[to_index(r, c)] = digit;
                col_count += 1;
            }

            if col_count != SIDE {
                return Err(SudokuError::InvalidColumnCount { row: r });
            }

            row_count += 1;
        }

        if row_count != SIDE {
            return Err(SudokuError::InvalidRowCount);
        }

        Sudoku::try_from_grid(grid)
    }
}

/// bit 0: solved flag; bits 1-9: candidate digits
type CandidateMask = u16;

#[inline]
fn single_digit_mask(digit: u8) -> CandidateMask {
    debug_assert!((1..=SIDE as u8).contains(&digit));
    1u16 << digit
}

const FULL_DIGIT_MASK: CandidateMask = (1u16 << (SIDE + 1)) - 2;

/// a list of the available valid candidates per cell
#[derive(Clone)]
struct CandidateList {
    valid_candidates: [CandidateMask; CELL_COUNT],
    empty_cells: usize,
}

impl CandidateList {
    const SOLVED_FLAG: CandidateMask = 1;

    #[inline]
    fn is_cell_solved(&self, index: usize) -> bool {
        self.valid_candidates[index] & Self::SOLVED_FLAG != 0
    }

    #[inline]
    fn is_digit_valid_candidate(&self, index: usize, digit: u8) -> bool {
        self.valid_candidates[index] & single_digit_mask(digit) != 0
    }

    #[inline]
    fn candidate_bits(&self, index: usize) -> CandidateMask {
        self.valid_candidates[index] & FULL_DIGIT_MASK
    }

    fn has_cell_without_candidates(&self) -> bool {
        (0..CELL_COUNT).any(|index| {
            !self.is_cell_solved(index) && self.candidate_bits(index) == 0
        })
    }

    /// checks whether placing `digit` at the given position conflicts with any
    /// peer or would eliminate the last valid candidate of a peer cell
    fn is_placement_inconsistent(&self, index: usize, digit: u8) -> bool {
        PEERS[index].iter().any(|&peer| {
            if self.is_cell_solved(peer) {
                self.is_digit_valid_candidate(peer, digit)
            } else {
                // peer is not yet marked solved, but it only has one candidate
                self.valid_candidates[peer] == single_digit_mask(digit)
            }
        })
    }

    #[inline]
    fn remove_candidate(&mut self, index: usize, digit: u8) {
        self.valid_candidates[index] &= !single_digit_mask(digit);
    }

    /// sets the value of the cell and updates the candidate lists
    /// of its peers
    /// it does NOT perform consistency checks
    fn set_cell_unsafe(&mut self, index: usize, digit: u8) {
        PEERS[index].iter().for_each(|&peer|
            self.remove_candidate(peer, digit)
        );

        self.valid_candidates[index] = single_digit_mask(digit) | Self::SOLVED_FLAG;
        self.empty_cells -= 1;
    }

    /// recursively eliminate candidates until a solution is reached
    fn eliminate_candidates(self) -> Option<Self> {
        if self.empty_cells == 0 {
            return Some(self);
        }

        let (cell, _) =
            (0..CELL_COUNT)
                .filter(|&n| !self.is_cell_solved(n))
                .map(|n| (n, self.candidate_bits(n).count_ones()))
                .min_by_key(|n| n.1)
                .expect("always exist >= 1");

        for digit in 1..=SIDE as u8 {
            if !self.is_digit_valid_candidate(cell, digit) {
                continue;
            }

            if self.is_placement_inconsistent(cell, digit) {
                continue;
            }

            let mut list = self.clone();
            list.set_cell_unsafe(cell, digit);

            if let Some(solution) = list.eliminate_candidates() {
                return Some(solution);
            }
        }

        None
    }
}

impl TryFrom<&Sudoku> for CandidateList {
    type Error = SudokuError;

    fn try_from(sudoku: &Sudoku) -> Result<Self, Self::Error> {
        let mut list = Self {
            valid_candidates: [FULL_DIGIT_MASK; CELL_COUNT],
            empty_cells: CELL_COUNT
        };

        for index in 0..CELL_COUNT {
            let digit = sudoku.grid[index];
            if digit == EMPTY_CELL {
                continue;
            }
            list.set_cell_unsafe(index, digit);
        }

        if !list.has_cell_without_candidates() {
            Ok(list)
        } else {
            Err(SudokuError::NoSolution)
        }
    }
}

impl From<&CandidateList> for Sudoku {
    fn from(candidate_list: &CandidateList) -> Self {
        let mut sudoku = Self::new_empty();

        for index in 0..CELL_COUNT {
            if !candidate_list.is_cell_solved(index) {
                continue;
            }

            let mask = candidate_list.valid_candidates[index];
            let digit = (mask - 1).trailing_zeros() as u8;
            sudoku.grid[index] = digit;
        }

        sudoku
    }
}






#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    const GRID1: [u8; CELL_COUNT] = [
        5, 3, 4, 0, 0, 8, 0, 1, 0,
        0, 0, 0, 0, 0, 2, 0, 9, 0,
        0, 0, 0, 0, 0, 7, 6, 0, 4,
        0, 0, 0, 5, 0, 0, 1, 0, 0,
        1, 0, 0, 0, 0, 0, 0, 0, 3,
        0, 0, 9, 0, 0, 1, 0, 0, 0,
        3, 0, 5, 4, 0, 0, 0, 0, 0,
        0, 8, 0, 2, 0, 0, 0, 0, 0,
        0, 6, 0, 7, 0, 0, 3, 8, 2,
    ];

    const GRID2: [u8; CELL_COUNT] = [
        0, 0, 3, 0, 2, 0, 6, 0, 0,
        9, 0, 0, 3, 0, 5, 0, 0, 1,
        0, 0, 1, 8, 0, 6, 4, 0, 0,
        0, 0, 8, 1, 0, 2, 9, 0, 0,
        7, 0, 0, 0, 0, 0, 0, 0, 8,
        0, 0, 6, 7, 0, 8, 2, 0, 0,
        0, 0, 2, 6, 0, 9, 5, 0, 0,
        8, 0, 0, 2, 0, 3, 0, 0, 9,
        0, 0, 5, 0, 1, 0, 3, 0, 0,
    ];

    const SOLVED_GRID: [u8; CELL_COUNT] = [
        5, 3, 4, 6, 7, 8, 9, 1, 2,
        6, 7, 2, 1, 9, 5, 3, 4, 8,
        1, 9, 8, 3, 4, 2, 5, 6, 7,
        8, 5, 9, 7, 6, 1, 4, 2, 3,
        4, 2, 6, 8, 5, 3, 7, 9, 1,
        7, 1, 3, 9, 2, 4, 8, 5, 6,
        9, 6, 1, 5, 3, 7, 2, 8, 4,
        2, 8, 7, 4, 1, 9, 6, 3, 5,
        3, 4, 5, 2, 8, 6, 1, 7, 9,
    ];

    const NO_SOLUTION_GRID: [u8; CELL_COUNT] = [
        0, 5, 4, 6, 7, 8, 9, 1, 2,
        6, 7, 2, 1, 9, 5, 3, 4, 8,
        1, 9, 8, 3, 4, 2, 5, 6, 7,
        8, 0, 9, 7, 6, 1, 4, 2, 3,
        4, 2, 6, 8, 5, 3, 7, 9, 1,
        7, 1, 3, 9, 2, 4, 8, 5, 6,
        9, 6, 1, 5, 3, 7, 2, 8, 4,
        2, 8, 7, 4, 1, 9, 6, 3, 5,
        3, 4, 5, 2, 8, 6, 1, 7, 9,
    ];

    const PUZZLE1: &str = "530070000\n600195000\n098000060\n800060003\n400803001\n700020006\n060000280\n000419005\n000080079";
    const PUZZLE2: &str = "003020600\n900305001\n001806400\n008102900\n700000008\n006708200\n002609500\n800203009\n005010300";

    #[test]
    fn empty_board_is_consistent_but_not_solved() {
        let sudoku = Sudoku::new_empty();
        assert!(sudoku.is_consistent());
        assert!(!sudoku.is_solved());
    }

    #[test]
    fn accepts_solved_grid() {
        let sudoku = Sudoku::try_from_grid(SOLVED_GRID).expect("");
        assert!(sudoku.is_consistent());
        assert!(sudoku.is_solved());
    }

    #[test]
    fn create_puzzle_from_grid() {
        for grid in [GRID1, GRID2, SOLVED_GRID, NO_SOLUTION_GRID] {
            assert!(Sudoku::try_from_grid(grid).is_ok(), "Failed to construct sudoku from grid");
        }
    }

    #[test]
    fn parse_puzzle_from_str() {
        assert!(Sudoku::try_from(PUZZLE1).is_ok(), "Failed to parse puzzle from str");
        assert!(Sudoku::try_from(PUZZLE2).is_ok(), "Failed to parse puzzle from str");
    }

    #[test]
    fn rejects_wrong_rows() {
        let puzzle = "123456789\n".repeat(5);
        assert_matches!(Sudoku::try_from(puzzle.as_str()), Err(SudokuError::InvalidRowCount));

        let puzzle = "123456789\n".repeat(10);
        assert_matches!(Sudoku::try_from(puzzle.as_str()), Err(SudokuError::InvalidRowCount));
    }

    #[test]
    fn rejects_wrong_columns() {
        let puzzle = "1\n12\n3456789\n".repeat(5);
        assert_matches!(Sudoku::try_from(puzzle.as_str()), Err(SudokuError::InvalidColumnCount { .. }));

        let puzzle = "123456789\n".repeat(8) + "123456\n";
        assert_matches!(Sudoku::try_from(puzzle.as_str()), Err(SudokuError::InvalidColumnCount { .. }));
    }

    #[test]
    fn rejects_digit_out_of_range() {
        let index = 0;
        let mut grid = GRID1;
        grid[index] = 10;
        assert_matches!(Sudoku::try_from_grid(grid), Err(SudokuError::InvalidClue { .. }));
    }

    #[test]
    fn rejects_invalid_digit() {
        let invalid_puzzle = format!("x{PUZZLE1}");
        assert_matches!(Sudoku::try_from(invalid_puzzle.as_str()), Err(SudokuError::InvalidClue { .. }));

        let invalid_puzzle = "1x3456789\n".repeat(9);
        assert_matches!(Sudoku::try_from(invalid_puzzle.as_str()), Err(SudokuError::InvalidClue { .. }));
    }

    #[test]
    fn rejects_duplicate_digit() {
        for index in 0..CELL_COUNT {
            for peer in PEERS[index] {
                let mut grid = GRID1;
                grid[index] = 5;
                grid[peer] = 5;
                assert_matches!(Sudoku::try_from_grid(grid), Err(SudokuError::InconsistentPuzzle));
            }
        }
    }

    #[test]
    fn solve_puzzles() {
        let puzzles = [
            Sudoku::try_from(PUZZLE1),
            Sudoku::try_from(PUZZLE2),
            Sudoku::try_from_grid(GRID1),
            Sudoku::try_from_grid(GRID2),
            Sudoku::try_from_grid(SOLVED_GRID),
        ].map(|puzzle| puzzle.expect("ok"));

        for puzzle in puzzles {
            let solved_puzzle = puzzle.clone().solve();

            assert!(solved_puzzle.is_ok(), "Failed to return a solution of\n{puzzle:#}");
            let solved_puzzle = solved_puzzle.expect("just asserted above");

            assert!(solved_puzzle.is_solved(), "The solution validation failed on\n{puzzle:#}");

            puzzle.grid.iter()
                .zip(solved_puzzle.grid.iter())
                .for_each(|(&d1, &d2)|
                    if d1 != 0 {
                        assert_eq!(d1, d2, "Solution has different clues than original puzzle")
                    }
                );
        }
    }

    #[test]
    fn no_solution_on_invalid_puzzle() {
        let sudoku = Sudoku::try_from_grid(NO_SOLUTION_GRID).expect("valid sudoku");
        assert_matches!(sudoku.solve(), Err(SudokuError::NoSolution));
    }
}