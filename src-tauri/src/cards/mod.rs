//! Data for the card widgets (Lumabar-style): what the PC knows by itself (screen time, Codex's
//! limits, network speed, temperature, files) and what public services answer without an
//! account (stocks, coins, currencies, GitHub and LeetCode by username).

pub mod local;
pub mod online;
pub mod screentime;
