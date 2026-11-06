//! The 3x3x3 last layer sets.
//!
//! The algorithms are the standard ones listed on `SpeedCubeDB`. The tests in
//! `trainer` check each of them against its set.

use super::{AlgCase, AlgSet, Step};

const fn case(name: &'static str, group: &'static str, algorithm: &'static str) -> AlgCase {
    AlgCase {
        name,
        group,
        algorithm,
    }
}

const EDGES: &str = "Edges only";
const CORNERS: &str = "Corners only";
const ADJACENT: &str = "Adjacent swap";
const DIAGONAL: &str = "Diagonal swap";
const G_PERMS: &str = "G perms";

/// The 21 cases of permuting the last layer.
pub static PLL: AlgSet = AlgSet {
    id: "pll",
    name: "PLL",
    step: Step::PermuteLastLayer,
    cases: &[
        case("Ua", EDGES, "y2 (M2 U M) U2 (M' U M2)"),
        case("Ub", EDGES, "y2 (M2 U' M) U2 (M' U' M2)"),
        case("H", EDGES, "(M2 U' M2) U2 (M2 U' M2)"),
        case("Z", EDGES, "(M2 U) (M2 U) (M' U2) M2 (U2 M')"),
        case("Aa", CORNERS, "x (R' U R') D2 (R U' R') D2 R2 x'"),
        case("Ab", CORNERS, "x R2 D2 (R U R') D2 (R U' R) x'"),
        case(
            "E",
            CORNERS,
            "y x' (R U' R' D) (R U R' D') (R U R' D) (R U' R' D') x",
        ),
        case(
            "T",
            ADJACENT,
            "(R U R' U') (R' F R2) (U' R' U') (R U R' F')",
        ),
        case(
            "F",
            ADJACENT,
            "y (R' U' F') (R U R' U') R' F R2 (U' R' U') (R U R' U) R",
        ),
        case("Ja", ADJACENT, "y (R' U L') U2 (R U' R') U2 R L"),
        case("Jb", ADJACENT, "(R U R' F') (R U R' U') R' F R2 U' R'"),
        case(
            "Ra",
            ADJACENT,
            "y (R U' R' U') (R U R D) (R' U' R D') (R' U2 R')",
        ),
        case("Rb", ADJACENT, "(R' U2) (R U2) (R' F R) (U R' U' R') F' R2"),
        case(
            "V",
            DIAGONAL,
            "(R' U R' U') (R D' R' D) (R' U D') (R2 U' R2) D R2",
        ),
        case(
            "Y",
            DIAGONAL,
            "F R (U' R' U') (R U R' F') (R U R' U') (R' F R F')",
        ),
        case(
            "Na",
            DIAGONAL,
            "(R U R' U) (R U R' F') (R U R' U') R' F R2 U' R' U2 (R U' R')",
        ),
        case(
            "Nb",
            DIAGONAL,
            "(R' U R U' R') (F' U' F) (R U R') (F R' F') (R U' R)",
        ),
        case("Ga", G_PERMS, "R2 (U R' U R' U' R U') R2 D (U' R' U R) D'"),
        case("Gb", G_PERMS, "(R' U' R U) D' R2 (U R' U R U' R U') R2 D"),
        case("Gc", G_PERMS, "R2 (U' R U' R U R' U) R2 D' (U R U' R') D"),
        case("Gd", G_PERMS, "(R U R' U') D R2 (U' R U' R' U R' U) R2 D'"),
    ],
};

const CROSS: &str = "All edges oriented";
const T_SHAPES: &str = "T shapes";
const SQUARES: &str = "Squares";
const C_SHAPES: &str = "C shapes";
const W_SHAPES: &str = "W shapes";
const CORNERS_ORIENTED: &str = "Corners oriented";
const P_SHAPES: &str = "P shapes";
const I_SHAPES: &str = "I shapes";
const FISH: &str = "Fish";
const KNIGHTS: &str = "Knights";
const AWKWARD: &str = "Awkward";
const L_SHAPES: &str = "L shapes";
const LIGHTNING: &str = "Lightning";
const DOTS: &str = "Dots";

/// The 57 cases of orienting the last layer.
pub static OLL: AlgSet = AlgSet {
    id: "oll",
    name: "OLL",
    step: Step::OrientLastLayer,
    cases: &[
        case("OLL 21", CROSS, "(R U R' U) (R U' R' U) (R U2 R')"),
        case("OLL 22", CROSS, "R U2 (R2' U') (R2 U') (R2' U') U' R"),
        case("OLL 23", CROSS, "R2 D (R' U2 R) D' (R' U2 R')"),
        case("OLL 24", CROSS, "(r U R' U') (r' F R F')"),
        case("OLL 25", CROSS, "y (F' r U R') (U' r' F R)"),
        case("OLL 26", CROSS, "y R U2 (R' U' R U') R'"),
        case("OLL 27", CROSS, "(R U R' U) (R U2 R')"),
        case("OLL 33", T_SHAPES, "(R U R' U') (R' F R F')"),
        case("OLL 45", T_SHAPES, "F (R U R' U') F'"),
        case("OLL 5", SQUARES, "r' U2 (R U R' U) r"),
        case("OLL 6", SQUARES, "r U2 (R' U' R U') r'"),
        case("OLL 34", C_SHAPES, "y2 R U R2 U' R' F (R U R U') F'"),
        case("OLL 46", C_SHAPES, "R' U' (R' F R F') U R"),
        case("OLL 36", W_SHAPES, "y2 (L' U' L U') (L' U L U) (L F' L' F)"),
        case("OLL 38", W_SHAPES, "(R U R' U) (R U' R' U') (R' F R F')"),
        case("OLL 28", CORNERS_ORIENTED, "(r U R' U') M (U R U' R')"),
        case("OLL 57", CORNERS_ORIENTED, "(R U R' U') M' (U R U' r')"),
        case("OLL 31", P_SHAPES, "(R' U' F) (U R U' R') F' R"),
        case("OLL 32", P_SHAPES, "S (R U R' U') (R' F R f')"),
        case("OLL 43", P_SHAPES, "y R' U' (F' U F) R"),
        case("OLL 44", P_SHAPES, "f (R U R' U') f'"),
        case("OLL 51", I_SHAPES, "f (R U R' U') (R U R' U') f'"),
        case("OLL 52", I_SHAPES, "y2 R' (F' U' F U') (R U R' U) R"),
        case("OLL 55", I_SHAPES, "R U2 R2 (U' R U' R') U2 (F R F')"),
        case(
            "OLL 56",
            I_SHAPES,
            "(r U r') (U R U' R') (U R U' R') (r U' r')",
        ),
        case("OLL 9", FISH, "y (R U R' U') (R' F R) (R U R' U') F'"),
        case("OLL 10", FISH, "(R U R' U) (R' F R F') (R U2 R')"),
        case("OLL 35", FISH, "(R U2 R') (R' F R F') (R U2 R')"),
        case("OLL 37", FISH, "F R (U' R' U') (R U R') F'"),
        case("OLL 13", KNIGHTS, "(r U' r') U' (r U r') (F' U F)"),
        case("OLL 14", KNIGHTS, "R' F (R U R') F' R (F U' F')"),
        case("OLL 15", KNIGHTS, "(r' U' r) (R' U' R U) (r' U r)"),
        case("OLL 16", KNIGHTS, "(r U r') (R U R' U') (r U' r')"),
        case(
            "OLL 29",
            AWKWARD,
            "y (R U R') U' (R U' R') (F' U' F) (R U R')",
        ),
        case("OLL 30", AWKWARD, "y2 F U (R U2 R') U' (R U2 R') U' F'"),
        case(
            "OLL 41",
            AWKWARD,
            "y2 (R U R' U) (R U2 R') F (R U R' U') F'",
        ),
        case("OLL 42", AWKWARD, "(R' U' R U') (R' U2 R) F (R U R' U') F'"),
        case("OLL 47", L_SHAPES, "F' (L' U' L U) (L' U' L U) F"),
        case("OLL 48", L_SHAPES, "F (R U R' U') (R U R' U') F'"),
        case("OLL 49", L_SHAPES, "y2 r U' (r2 U) (r2 U) (r2) U' r"),
        case("OLL 50", L_SHAPES, "r' U (r2 U') (r2 U') (r2) U r'"),
        case("OLL 53", L_SHAPES, "(r' U' R U') (R' U R U') (R' U2 r)"),
        case("OLL 54", L_SHAPES, "(r U R' U) (R U' R' U) (R U2 r')"),
        case("OLL 7", LIGHTNING, "r (U R' U R) U2 r'"),
        case("OLL 8", LIGHTNING, "y2 r' (U' R U' R') U2 r"),
        case("OLL 11", LIGHTNING, "M (R U R' U R U2 R') U M'"),
        case("OLL 12", LIGHTNING, "y' M' (R' U' R U' R' U2 R) U' M"),
        case("OLL 39", LIGHTNING, "y L F' (L' U' L U) F U' L'"),
        case("OLL 40", LIGHTNING, "y R' F (R U R' U') F' U R"),
        case("OLL 1", DOTS, "(R U2 R') (R' F R F') U2 (R' F R F')"),
        case("OLL 2", DOTS, "F (R U R' U') F' f (R U R' U') f'"),
        case("OLL 3", DOTS, "y' f (R U R' U') f' (U') F (R U R' U') F'"),
        case("OLL 4", DOTS, "y' f (R U R' U') f' (U) F (R U R' U') F'"),
        case("OLL 17", DOTS, "(R U R' U) (R' F R F') U2 (R' F R F')"),
        case("OLL 18", DOTS, "y (R U2 R') (R' F R F') U2 M' (U R U' r')"),
        case("OLL 19", DOTS, "M U (R U R' U') M' (R' F R F')"),
        case("OLL 20", DOTS, "(r U R' U') M2 (U R U' R') U' M'"),
    ],
};
