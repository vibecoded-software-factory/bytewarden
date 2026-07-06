//! Vault list state — the app's core view-model.
//!
//! The item data, the derived search/filter caches, the sidebar
//! counts and the list-navigation cursor, split out of the
//! [`crate::tui::app::App`] god-struct into one container that owns
//! its own **invalidation contract**: the `rebuild_*` methods that keep
//! the caches consistent live next to the fields they protect, so
//! "mutating an input without calling the matching rebuild" is a local
//! concern rather than a footgun spread across the whole app.
//!
//! Selection always indexes the **filtered** cache (never the raw vec)
//! and is re-anchored by **id** after a wholesale reload — see
//! [`Vault::reanchor_selection`].

use std::collections::HashMap;

use crate::domain::LoweredItem;
use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::item::Item;
use crate::tui::app::{PAGE_STEP, VAULT_VIEWPORT_ROWS, compute_filtered_indices};
use crate::tui::folders::FolderFilter;

/// The vault list: item data + derived caches + list navigation state.
pub struct Vault {
    /// Active item-type filter (All / Favorites / a type / Trash).
    pub active_filter: ItemFilter,
    /// Highlight index in the item-type sidebar.
    pub filter_selected: usize,
    /// The full personal + org vault (as last loaded / synced).
    pub items: Vec<Item>,
    /// Cursor into the **filtered** cache (never the raw vec).
    pub selected_index: usize,
    /// First visible row — kept consistent with `selected_index`.
    pub scroll_offset: usize,
    /// Trashed items — fetched on demand when [`ItemFilter::Trash`] is
    /// selected.
    pub trashed_items: Vec<Item>,
    /// Pre-lowercased projection of [`Self::items`], kept parallel and
    /// the same length. Refreshed by [`Self::rebuild_search_caches`] so
    /// the search hot path doesn't allocate one lowercased string per
    /// item per keystroke.
    pub items_lowered: Vec<LoweredItem>,
    /// Same idea as [`Self::items_lowered`] but for the trash view.
    pub trashed_lowered: Vec<LoweredItem>,
    /// Indices into [`Self::items`] or [`Self::trashed_items`] surviving
    /// the active type + folder + search filters, ranked by fuzzy score
    /// when a query is active. Read by [`Self::filtered_items`] in O(K).
    pub filtered_cache: Vec<usize>,
    /// Number of items whose `folder_id` is `None`. Cached so the
    /// folders sidebar renders the "(No folder)" badge in O(1).
    pub no_folder_count: usize,
    /// `folder_id → item count`, cached for O(1) sidebar badges.
    pub folder_counts: HashMap<String, usize>,
    /// `collection_id → item count`, cached for O(1) sidebar badges.
    pub collection_counts: HashMap<String, usize>,
    /// Active folder/collection filter (ANDed with `active_filter`).
    pub active_folder: FolderFilter,
    /// Highlight index in the Folders sidebar panel.
    pub folder_selected: usize,
    /// The search-box text (drives the fuzzy ranking).
    pub search_query: String,
}

impl Default for Vault {
    fn default() -> Self {
        Self {
            active_filter: ItemFilter::All,
            filter_selected: 0,
            items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            trashed_items: Vec::new(),
            items_lowered: Vec::new(),
            trashed_lowered: Vec::new(),
            filtered_cache: Vec::new(),
            no_folder_count: 0,
            folder_counts: HashMap::new(),
            collection_counts: HashMap::new(),
            active_folder: FolderFilter::All,
            folder_selected: 0,
            search_query: String::new(),
        }
    }
}

impl Vault {
    /// Shorthand — `true` when the active filter is [`ItemFilter::Trash`].
    pub fn is_trash_view(&self) -> bool {
        self.active_filter == ItemFilter::Trash
    }

    // ── Filter / list movement ────────────────────────────────────────────

    pub fn filter_move_down(&mut self) {
        if self.filter_selected < ITEM_FILTERS.len() - 1 {
            self.filter_selected += 1;
        }
    }
    pub fn filter_move_up(&mut self) {
        if self.filter_selected > 0 {
            self.filter_selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_items().len();
        if len > 0 && self.selected_index < len - 1 {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + VAULT_VIEWPORT_ROWS {
                self.scroll_offset += 1;
            }
        }
    }
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }
    pub fn move_down_page(&mut self) {
        for _ in 0..PAGE_STEP {
            self.move_down();
        }
    }
    pub fn move_up_page(&mut self) {
        for _ in 0..PAGE_STEP {
            self.move_up();
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────

    /// References to the items that should currently be visible in the
    /// main list, after the active type + folder filters and the
    /// search-box ranking. O(K) — one indirection per visible row.
    pub fn filtered_items(&self) -> Vec<&Item> {
        let source = if self.active_filter == ItemFilter::Trash {
            &self.trashed_items
        } else {
            &self.items
        };
        self.filtered_cache
            .iter()
            .filter_map(|&i| source.get(i))
            .collect()
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.filtered_items().get(self.selected_index).copied()
    }

    /// Id of the currently selected (filtered) item, if any — captured
    /// before a reload so the cursor can be put back on the same item.
    pub fn selected_item_id(&self) -> Option<String> {
        self.selected_item().map(|i| i.id.clone())
    }

    /// Re-anchors the list cursor onto the item with `id` after a reload
    /// that may have reordered or replaced the list. Falls back to
    /// clamping the old index into range when the item is gone. Keeps
    /// `scroll_offset` consistent so the cursor stays visible. This is
    /// the invalidation contract: a background/post-mutation refresh
    /// must never yank the cursor onto an unrelated row just because
    /// indices shifted.
    pub fn reanchor_selection(&mut self, id: Option<&str>) {
        let len = self.filtered_items().len();
        self.selected_index = match id {
            Some(id) => self
                .filtered_items()
                .iter()
                .position(|i| i.id == id)
                .unwrap_or_else(|| self.selected_index.min(len.saturating_sub(1))),
            None => self.selected_index.min(len.saturating_sub(1)),
        };
        if len == 0 {
            self.selected_index = 0;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Item count for a given filter — used to render sidebar badges.
    pub fn count_for(&self, filter: &ItemFilter) -> usize {
        match filter {
            ItemFilter::All => self.items.len(),
            ItemFilter::Favorites => self.items.iter().filter(|i| i.favorite).count(),
            ItemFilter::Trash => self.trashed_items.len(),
            f => self
                .items
                .iter()
                .filter(|i| f.type_id() == Some(i.item_type))
                .count(),
        }
    }

    // ── Invalidation contract (rebuilds) ──────────────────────────────────

    /// Rebuilds the lowered projections from the item vecs. Called after
    /// any mutation that could touch a searchable field. Always pairs
    /// with a [`Self::rebuild_filtered_cache`] — the filtered cache
    /// references items by index.
    pub fn rebuild_search_caches(&mut self) {
        self.items_lowered = self.items.iter().map(LoweredItem::from_item).collect();
        self.trashed_lowered = self
            .trashed_items
            .iter()
            .map(LoweredItem::from_item)
            .collect();
    }

    /// Recomputes [`Self::filtered_cache`] from the current items,
    /// active filters and search query.
    pub fn rebuild_filtered_cache(&mut self) {
        let (source, lowered): (&[Item], &[LoweredItem]) =
            if self.active_filter == ItemFilter::Trash {
                (&self.trashed_items, &self.trashed_lowered)
            } else {
                (&self.items, &self.items_lowered)
            };
        self.filtered_cache = compute_filtered_indices(
            source,
            lowered,
            &self.active_filter,
            &self.active_folder,
            &self.search_query,
        );
    }

    /// Rebuilds every cache in the order required by their invariants
    /// (lowered first — filtered references the lowered vec for scoring).
    /// Use from any mutation that replaces items wholesale or might have
    /// altered a searchable field.
    pub fn rebuild_caches(&mut self) {
        self.rebuild_search_caches();
        self.rebuild_filtered_cache();
        self.rebuild_sidebar_counts();
    }

    /// Rebuilds the sidebar count maps from the current items in one
    /// O(N) pass. Folders / collections that no longer have any items
    /// are dropped from the maps (the renderer treats a missing key as
    /// zero).
    pub fn rebuild_sidebar_counts(&mut self) {
        self.no_folder_count = 0;
        self.folder_counts.clear();
        self.collection_counts.clear();
        for item in &self.items {
            match item.folder_id.as_deref() {
                None => self.no_folder_count += 1,
                Some(id) => *self.folder_counts.entry(id.to_string()).or_insert(0) += 1,
            }
            for cid in &item.collection_ids {
                *self.collection_counts.entry(cid.clone()).or_insert(0) += 1;
            }
        }
    }

    /// Snaps the cursor to the top and rebuilds the filtered cache —
    /// called when the search query changes.
    pub fn perform_search(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.rebuild_filtered_cache();
    }

    /// Sorts the in-memory vault list alphabetically (case-insensitive)
    /// and rebuilds the caches (the stored indices are now stale).
    pub fn sort_items(&mut self) {
        self.items.sort_by_cached_key(|i| i.name.to_lowercase());
        self.rebuild_caches();
    }
}
