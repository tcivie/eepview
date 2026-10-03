// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The context menu model (docs/wiki/links-and-shortcuts.md, C3 to C10 and P1 to P3). eepview
//! builds every menu from these 18 items only; no engine or system item is ever added. Pure.

use crate::nav::is_allowed;

/// One context menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemId {
    /// Open Link in New Tab.
    OpenLinkInNewTab,
    /// Open Link in New Background Tab.
    OpenLinkInBackgroundTab,
    /// Copy Link Address.
    CopyLinkAddress,
    /// Open Image in New Tab.
    OpenImageInNewTab,
    /// Copy Image Address.
    CopyImageAddress,
    /// Copy Image.
    CopyImage,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Cut.
    Cut,
    /// Copy.
    Copy,
    /// Paste.
    Paste,
    /// Select All.
    SelectAll,
    /// Back.
    Back,
    /// Forward.
    Forward,
    /// Reload.
    Reload,
    /// Bookmark This Page.
    BookmarkPage,
    /// Copy Page Address.
    CopyPageAddress,
    /// Find.
    Find,
}

impl ItemId {
    /// Every item, once.
    pub const ALL: [Self; 18] = [
        Self::OpenLinkInNewTab,
        Self::OpenLinkInBackgroundTab,
        Self::CopyLinkAddress,
        Self::OpenImageInNewTab,
        Self::CopyImageAddress,
        Self::CopyImage,
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::Back,
        Self::Forward,
        Self::Reload,
        Self::BookmarkPage,
        Self::CopyPageAddress,
        Self::Find,
    ];

    /// The string id (the menu item id).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.names().0
    }

    /// The label.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.names().1
    }

    /// The item of a string id.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.as_str() == id)
    }

    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::OpenLinkInNewTab => ("open-link-new-tab", "Open Link in New Tab"),
            Self::OpenLinkInBackgroundTab => (
                "open-link-background-tab",
                "Open Link in New Background Tab",
            ),
            Self::CopyLinkAddress => ("copy-link-address", "Copy Link Address"),
            Self::OpenImageInNewTab => ("open-image-new-tab", "Open Image in New Tab"),
            Self::CopyImageAddress => ("copy-image-address", "Copy Image Address"),
            Self::CopyImage => ("copy-image", "Copy Image"),
            other => other.edit_names(),
        }
    }

    fn edit_names(self) -> (&'static str, &'static str) {
        match self {
            Self::Undo => ("undo", "Undo"),
            Self::Redo => ("redo", "Redo"),
            Self::Cut => ("cut", "Cut"),
            Self::Copy => ("copy", "Copy"),
            Self::Paste => ("paste", "Paste"),
            Self::SelectAll => ("select-all", "Select All"),
            other => other.page_names(),
        }
    }

    fn page_names(self) -> (&'static str, &'static str) {
        match self {
            Self::Back => ("back", "Back"),
            Self::Forward => ("forward", "Forward"),
            Self::Reload => ("reload", "Reload"),
            Self::BookmarkPage => ("bookmark-page", "Bookmark This Page"),
            Self::CopyPageAddress => ("copy-page-address", "Copy Page Address"),
            _ => ("find", "Find"),
        }
    }
}

/// One menu entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    /// An item, enabled or not.
    Item {
        /// The item.
        id: ItemId,
        /// False shows it greyed out.
        enabled: bool,
    },
    /// A separator line.
    Separator,
}

/// What is under the pointer when the menu opens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// The absolute URL of the link under the pointer.
    pub link: Option<String>,
    /// The absolute URL of the image under the pointer.
    pub image: Option<String>,
    /// Text is selected.
    pub selection: bool,
    /// The pointer is in a text field.
    pub editable: bool,
    /// The URL of the page.
    pub page: String,
    /// Where the tab can go on its history.
    pub history: PageHistory,
    /// The page has a bookmark.
    pub bookmarked: bool,
}

/// Where a tab can go on its history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageHistory {
    /// The tab can go back.
    pub can_back: bool,
    /// The tab can go forward.
    pub can_forward: bool,
}

fn item(id: ItemId, enabled: bool) -> Entry {
    Entry::Item { id, enabled }
}

/// The menu for `target`, in order (C3 to C9).
#[must_use]
pub fn context_menu(target: &Target) -> Vec<Entry> {
    if target.editable {
        return editable(target.selection);
    }
    let groups: Vec<Vec<Entry>> = [
        target.link.as_deref().map(link_group),
        target.image.as_deref().map(image_group),
        target.selection.then(|| vec![item(ItemId::Copy, true)]),
    ]
    .into_iter()
    .flatten()
    .collect();
    if groups.is_empty() {
        return page_group(target);
    }
    groups.join(&Entry::Separator)
}

fn editable(selection: bool) -> Vec<Entry> {
    vec![
        item(ItemId::Undo, true),
        item(ItemId::Redo, true),
        Entry::Separator,
        item(ItemId::Cut, selection),
        item(ItemId::Copy, selection),
        item(ItemId::Paste, true),
        Entry::Separator,
        item(ItemId::SelectAll, true),
    ]
}

fn link_group(url: &str) -> Vec<Entry> {
    let open = is_allowed(url);
    vec![
        item(ItemId::OpenLinkInNewTab, open),
        item(ItemId::OpenLinkInBackgroundTab, open),
        item(ItemId::CopyLinkAddress, true),
    ]
}

fn image_group(url: &str) -> Vec<Entry> {
    let open = is_allowed(url);
    vec![
        item(ItemId::OpenImageInNewTab, open),
        item(ItemId::CopyImageAddress, true),
        item(ItemId::CopyImage, open),
    ]
}

fn page_group(target: &Target) -> Vec<Entry> {
    vec![
        item(ItemId::Back, target.history.can_back),
        item(ItemId::Forward, target.history.can_forward),
        item(ItemId::Reload, true),
        Entry::Separator,
        item(
            ItemId::BookmarkPage,
            is_allowed(&target.page) && !target.bookmarked,
        ),
        item(ItemId::CopyPageAddress, true),
        Entry::Separator,
        item(ItemId::Find, true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_ids_map_back_to_items() {
        for id in ItemId::ALL {
            assert_eq!(ItemId::parse(id.as_str()), Some(id));
        }
        assert_eq!(ItemId::parse("inspect-element"), None);
    }
}
