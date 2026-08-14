//! Tray items, and the filter that decides which ones show — section 4.2.
//!
//! The tray lives in the modeline, in the right cluster, to the left of the notification
//! icon. There is no separate bar and no popup (invariant 14).
//!
//! # Why filterable
//!
//! A typical desktop accumulates six or seven tray items nobody looks at. In the
//! modeline they contend for columns with the buffer's name. The filter is what
//! lets the tray exist without eating the line: by default **nothing shows**, and
//! the user lists what they want to see.

use std::collections::BTreeSet;

/// An item announced by an application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The service's id, unique. It comes from the `StatusNotifierItem`.
    pub id: String,
    /// A readable title, when the app provides one.
    pub title: String,
    /// The `IconName` requested by the app. It enters the icon chain.
    pub icon: Option<String>,
    /// Whether the app declares itself in a state of attention.
    pub atencao: bool,
}

/// The policy for which items show.
///
/// **An allow list, not a block list.** A tray that shows everything by
/// default is a tray that eats the modeline the day the user installs one more
/// application — and they would not connect the dots.
#[derive(Debug, Default)]
pub struct FilterOf {
    allowed: BTreeSet<String>,
    /// An item in a state of attention shows even without being on the list.
    ///
    /// It is the only hole, and it exists because the alternative is worse: an application
    /// asking for attention that the user never sees because they forgot to allow it.
    pub always_attention: bool,
}

impl FilterOf {
    /// A filter that shows nothing, except attention.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            allowed: BTreeSet::new(),
            always_attention: true,
        }
    }

    /// Allows an id.
    pub fn permit(&mut self, id: impl Into<String>) {
        self.allowed.insert(id.into());
    }

    /// Removes an id from the list.
    pub fn remove(&mut self, id: &str) {
        self.allowed.remove(id);
    }

    /// Does this item show?
    #[must_use]
    pub fn passes_through(&self, item: &Item) -> bool {
        if self.always_attention && item.atencao {
            return true;
        }
        self.allowed.contains(&item.id)
    }

    /// The visible items, in stable id order.
    ///
    /// A stable order matters: a tray that reorders itself makes the user
    /// click the wrong item.
    #[must_use]
    pub fn visible<'a>(&self, items: &'a [Item]) -> Vec<&'a Item> {
        let mut v: Vec<&Item> = items.iter().filter(|i| self.passes_through(i)).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, atencao: bool) -> Item {
        Item {
            id: id.into(),
            title: id.into(),
            icon: None,
            atencao,
        }
    }

    #[test]
    fn by_default_nothing_shows_up() {
        // An allow list. A tray that shows everything eats the modeline the day
        // the user installs one more application.
        let f = FilterOf::fresh();
        assert!(f.visible(&[item("a", false), item("b", false)]).is_empty());
    }

    #[test]
    fn attention_cuts_through_the_filter() {
        // The alternative is worse: an application asking for attention that the user
        // never sees because they forgot to allow it.
        let f = FilterOf::fresh();
        assert_eq!(f.visible(&[item("a", true)]).len(), 1);
    }

    #[test]
    fn attention_can_be_switched_off() {
        let mut f = FilterOf::fresh();
        f.always_attention = false;
        assert!(f.visible(&[item("a", true)]).is_empty());
    }

    #[test]
    fn the_order_is_stable() {
        // A tray that reorders itself makes the user click the wrong item.
        let mut f = FilterOf::fresh();
        for id in ["z", "a", "m"] {
            f.permit(id);
        }
        let items = [item("m", false), item("z", false), item("a", false)];
        let v: Vec<_> = f.visible(&items).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(v, ["a", "m", "z"]);
    }

    #[test]
    fn removing_takes_it_off_the_list() {
        let mut f = FilterOf::fresh();
        f.permit("a");
        assert_eq!(f.visible(&[item("a", false)]).len(), 1);
        f.remove("a");
        assert!(f.visible(&[item("a", false)]).is_empty());
    }
}
