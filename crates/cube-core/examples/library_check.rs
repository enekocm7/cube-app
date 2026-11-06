//! Checks every algorithm in the library against its puzzle and lists those
//! that fail, with how many cases and algorithms each set has.
//!
//! ```sh
//! cargo run --release -p cube-core --example library_check
//! ```

use cube_core::library::{case_state, check, library_sets};

fn main() {
    let mut failures = 0;
    for set in library_sets() {
        let mut algorithms = 0;
        let mut unset = 0;
        for case in set.cases {
            if case_state(set, case).is_none() {
                println!("UNSET\t{}\t{}\t{}", set.id, case.name, case.algorithm);
                unset += 1;
                failures += 1;
                continue;
            }
            for algorithm in case.algorithms {
                algorithms += 1;
                if let Err(error) = check(set, case, algorithm) {
                    println!("FAIL\t{}\t{}\t{algorithm}\t{error:?}", set.id, case.name);
                    failures += 1;
                }
            }
        }
        eprintln!(
            "{:<10} {:<14} {:>4} cases {:>5} algorithms {:>3} unset",
            set.event.name(),
            set.id,
            set.cases.len(),
            algorithms,
            unset
        );
    }
    eprintln!("{failures} failures");
}
