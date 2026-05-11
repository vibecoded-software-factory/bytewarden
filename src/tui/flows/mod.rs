//! Multi-step user flows that mutate [`crate::tui::App`] and call into
//! the [`crate::ports`] adapters.
//!
//! Each sub-module owns one logical area of the application. Free
//! functions take `&mut App` so they can be dispatched from the
//! pending-action queue without method-resolution gymnastics.

pub mod assign_collections;
pub mod auth;
pub mod copy;
pub mod export;
pub mod folders;
pub mod generator;
pub mod import;
pub mod item_json;
pub mod items;
pub mod memberships;
pub mod reprompt;
pub mod send;
pub mod vault;
