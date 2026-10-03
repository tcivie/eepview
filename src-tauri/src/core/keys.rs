//! Keyboard shortcuts (menu accelerators) to commands.

use super::find::Zoom;
use super::{Core, Effect, Event};
use crate::session::Step;
use crate::shortcuts::Action;
use crate::tabs::Place;

impl Core {
    /// Runs a shortcut.
    pub fn shortcut(&mut self, action: Action, now: u64) -> Vec<Effect> {
        match action {
            Action::NewTab => self.tab_new(None, Place::End).1,
            Action::CloseTab => self.tab_close(self.tabs.active_id()),
            Action::ReopenTab => self.tab_reopen(),
            Action::NextTab => self.tab_cycle(true),
            Action::PrevTab => self.tab_cycle(false),
            Action::SelectTab(n) => self.tab_number(n),
            Action::Back => self.step(Step::Back),
            Action::Forward => self.step(Step::Forward),
            Action::Reload => self.reload(false),
            Action::HardReload => self.reload(true),
            Action::Stop => self.stop(),
            Action::Home => self.home(),
            other => self.ui_shortcut(other, now),
        }
    }

    fn ui_shortcut(&mut self, action: Action, now: u64) -> Vec<Effect> {
        match action {
            Action::FocusAddress => ui("focus-address"),
            Action::Find => {
                self.find_open = true;
                let mut fx = ui("open-find");
                fx.push(Effect::Layout);
                fx
            }
            Action::FindNext => self.find_again(true),
            Action::FindPrev => self.find_again(false),
            Action::Bookmark => self.bookmark_toggle(now),
            Action::Bookmarks => self.navigate("eepview://bookmarks").1,
            Action::History => self.navigate("eepview://history").1,
            Action::Settings => self.navigate("eepview://settings").1,
            Action::ZoomIn => self.zoom(Zoom::In),
            Action::ZoomOut => self.zoom(Zoom::Out),
            _ => self.zoom(Zoom::Reset),
        }
    }

    fn find_again(&mut self, forward: bool) -> Vec<Effect> {
        match self.find.as_ref().map(|f| (f.query.clone(), f.match_case)) {
            Some((query, case)) => self.find(&query, forward, case),
            None => self.ui_shortcut(crate::shortcuts::Action::Find, 0),
        }
    }
}

fn ui(action: &'static str) -> Vec<Effect> {
    vec![Effect::Emit(Event::Shortcut(action)), Effect::FocusToolbar]
}
