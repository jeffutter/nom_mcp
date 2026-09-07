//! Meal Type — the breakfast/lunch/dinner attribute of a logged Meal.
//!
//! A Meal carries exactly one [`MealType`]. When the caller doesn't supply
//! one, it is derived from the *local* wall-clock hour of the logged instant
//! in the shared [`crate::clock::Clock`] timezone — the same projection that
//! materializes `logged_date`. The derivation happens once at write time and
//! is never recomputed retroactively; pre-existing rows keep NULL.
//!
//! Default windows are half-open, gapless, and wrap midnight:
//! breakfast 05:00–10:59, lunch 11:00–15:59, dinner 16:00–04:59.

use std::ops::RangeInclusive;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which meal a logged Meal represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
}

/// Local hours whose meals default to breakfast (05:00–10:59).
const BREAKFAST_HOURS: RangeInclusive<u32> = 5..=10;
/// Local hours whose meals default to lunch (11:00–15:59).
const LUNCH_HOURS: RangeInclusive<u32> = 11..=15;

impl MealType {
    /// Derive the default type from a local wall-clock hour (0..=23).
    ///
    /// Everything outside the breakfast and lunch windows — including the
    /// post-midnight stretch — is dinner, which keeps the mapping exhaustive
    /// without a third explicit range.
    pub fn from_local_hour(hour: u32) -> Self {
        if BREAKFAST_HOURS.contains(&hour) {
            MealType::Breakfast
        } else if LUNCH_HOURS.contains(&hour) {
            MealType::Lunch
        } else {
            MealType::Dinner
        }
    }

    /// Parse a user-supplied value. Lenient by design: callers map `None` to
    /// an `ErrorData::validation` naming the offending field.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "breakfast" => Some(MealType::Breakfast),
            "lunch" => Some(MealType::Lunch),
            "dinner" => Some(MealType::Dinner),
            _ => None,
        }
    }

    /// Canonical lowercase string for storage and display.
    pub fn as_str(&self) -> &'static str {
        match self {
            MealType::Breakfast => "breakfast",
            MealType::Lunch => "lunch",
            MealType::Dinner => "dinner",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_hour_maps_to_exactly_one_variant() {
        for hour in 0..=23 {
            let derived = MealType::from_local_hour(hour);
            // Round-trip through the canonical string proves each hour lands
            // on a real variant and that as_str/parse agree.
            assert!(MealType::parse(derived.as_str()) == Some(derived));
        }
    }

    #[test]
    fn test_window_boundaries() {
        // Half-open windows, wrapping midnight: 04:59-class hour is dinner,
        // 05:00-class flips to breakfast, etc.
        assert_eq!(MealType::from_local_hour(4), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(0), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(5), MealType::Breakfast);
        assert_eq!(MealType::from_local_hour(10), MealType::Breakfast);
        assert_eq!(MealType::from_local_hour(11), MealType::Lunch);
        assert_eq!(MealType::from_local_hour(15), MealType::Lunch);
        assert_eq!(MealType::from_local_hour(16), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(23), MealType::Dinner);
    }

    #[test]
    fn test_full_hour_map() {
        let breakfast: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Breakfast)
            .collect();
        let lunch: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Lunch)
            .collect();
        let dinner: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Dinner)
            .collect();
        assert_eq!(breakfast, vec![5, 6, 7, 8, 9, 10]);
        assert_eq!(lunch, vec![11, 12, 13, 14, 15]);
        assert_eq!(dinner, vec![0, 1, 2, 3, 4, 16, 17, 18, 19, 20, 21, 22, 23]);
    }

    #[test]
    fn test_parse_accepts_only_canonical_values() {
        assert_eq!(MealType::parse("breakfast"), Some(MealType::Breakfast));
        assert_eq!(MealType::parse("lunch"), Some(MealType::Lunch));
        assert_eq!(MealType::parse("dinner"), Some(MealType::Dinner));
        assert_eq!(MealType::parse("Breakfast"), None);
        assert_eq!(MealType::parse("snack"), None);
        assert_eq!(MealType::parse(""), None);
    }

    #[test]
    fn test_serde_round_trips_lowercase() {
        let json = serde_json::to_string(&MealType::Breakfast).unwrap();
        assert_eq!(json, "\"breakfast\"");
        let back: MealType = serde_json::from_str("\"dinner\"").unwrap();
        assert_eq!(back, MealType::Dinner);
    }
}
