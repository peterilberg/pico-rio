cfg_if::cfg_if! {
    if #[cfg(feature = "water_tank")] {
        mod water_tank;
        pub use water_tank::*;
    } else {
        mod none;
        pub use none::*;
    }
}
