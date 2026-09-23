use std::collections::HashMap;

use crate::domain::filter::{ITEM_FILTERS, ItemFilter};
use crate::domain::item::Item;
use crate::domain::{LineEditor, LoweredItem};
use crate::tui::app::{PAGE_STEP, VAULT_VIEWPORT_ROWS, compute_filtered_indices};
use crate::tui::folders::FolderFilter;

pub struct Vault {
    pub active_filter: ItemFilter,

    pub filter_selected: usize,

    pub items: Vec<Item>,

    pub selected_index: usize,

    pub scroll_offset: usize,

    pub trashed_items: Vec<Item>,

    pub items_lowered: Vec<LoweredItem>,

    pub trashed_lowered: Vec<LoweredItem>,

    pub filtered_cache: Vec<usize>,

    pub no_folder_count: usize,

    pub folder_counts: HashMap<String, usize>,

    pub collection_counts: HashMap<String, usize>,

    pub active_folder: FolderFilter,

    pub folder_selected: usize,

    pub search_query: LineEditor,
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
            search_query: LineEditor::new(),
        }
    }
}

impl Vault {
    pub fn is_trash_view(&self) -> bool {
        self.active_filter == ItemFilter::Trash
    }

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

    pub fn selected_item_id(&self) -> Option<String> {
        self.selected_item().map(|i| i.id.clone())
    }

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

    pub fn rebuild_search_caches(&mut self) {
        self.items_lowered = self.items.iter().map(LoweredItem::from_item).collect();
        self.trashed_lowered = self
            .trashed_items
            .iter()
            .map(LoweredItem::from_item)
            .collect();
    }

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
            self.search_query.text(),
        );
    }

    pub fn rebuild_caches(&mut self) {
        self.rebuild_search_caches();
        self.rebuild_filtered_cache();
        self.rebuild_sidebar_counts();
    }

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

    pub fn perform_search(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.rebuild_filtered_cache();
    }

    pub fn sort_items(&mut self) {
        self.items.sort_by_cached_key(|i| i.name.to_lowercase());
        self.rebuild_caches();
    }
}
