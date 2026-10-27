use std::fs;
use std::path::PathBuf;

use clap::Parser;

use sudoku::{Sudoku, SudokuError};

/// Solve sudoku puzzles
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = "Solve the sudoku puzzle provided the input file

Expected puzzle format:
  - 9 rows, each containing 9 characters
  - Digits 1-9 represent filled cells
  - The digit 0 represents empty cells"
)]
struct Opt {
    /// Path to the input sudoku file
    input: PathBuf,

    /// Print the solution without grid separators
    #[arg(long)]
    nogrid: bool,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("failed to read input file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Solver(#[from] SudokuError),
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let opt = Opt::parse();

    let input = fs::read_to_string(&opt.input).map_err(|source| AppError::ReadFile {
        path: opt.input,
        source,
    })?;

    let sudoku = Sudoku::try_from(input.as_str())?;
    let solved_sudoku = sudoku.solve()?;

    if opt.nogrid {
        println!("{solved_sudoku}");
    } else {
        println!("{solved_sudoku:#}");
    }

    Ok(())
}
