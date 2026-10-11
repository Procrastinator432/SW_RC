//! Prepared native CalculateDamageFrom block (engine.dll 104f83f3..104f8562).
//! VM argument evaluation, UClass/CDO lookup and default-array construction are
//! host boundaries. Class ids must refer to one consistent prepared catalog.

#[derive(Debug, Clone, Copy)]
pub struct Include {
    /// None is the original null class wildcard, not an unresolved reference.
    pub class: Option<u32>,
    pub multiplier: f32,
}
#[derive(Debug)]
pub struct Variance<'a> {
    pub include: &'a [Include],
    pub exclude: &'a [Option<u32>],
}

/// An explicit invalid/missing default object suppresses actor fallback too.
#[derive(Debug, Clone, Copy)]
pub enum Binding<'a> {
    Valid(&'a Variance<'a>),
    Invalid,
}

/// `damage_type` is the valid DamageType default actor's class ancestry, most
/// derived first. None represents a null DamageType argument, not unknown data.
/// Explicit variance takes precedence even if invalid. First Include match wins
/// over all exclusions; unmatched defaults to one. Multiplication remains f32.
pub fn calculate(
    damage: f32,
    damage_type: Option<&[u32]>,
    actor: Binding<'_>,
    explicit: Option<Binding<'_>>,
) -> f32 {
    let mut multiplier = 1.;
    if let (Some(ancestry), Binding::Valid(variance)) = (damage_type, explicit.unwrap_or(actor)) {
        let matches = |class: Option<u32>| class.is_none_or(|id| ancestry.contains(&id));
        if let Some(rule) = variance.include.iter().find(|rule| matches(rule.class)) {
            multiplier = rule.multiplier;
        } else if variance.exclude.iter().any(|&class| matches(class)) {
            multiplier = 0.;
        }
    }
    multiplier * damage
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_inherited_include_precedes_more_specific_include_and_exclusion() {
        let rules = [
            Include {
                class: Some(2),
                multiplier: 0.5,
            },
            Include {
                class: Some(1),
                multiplier: 3.,
            },
        ];
        let v = Variance {
            include: &rules,
            exclude: &[Some(1)],
        };
        assert_eq!(
            calculate(20., Some(&[1, 2, 3]), Binding::Valid(&v), None),
            10.
        );
    }
    #[test]
    fn exclusions_use_ancestry_and_unmatched_damage_is_unchanged() {
        let v = Variance {
            include: &[],
            exclude: &[Some(2)],
        };
        assert_eq!(
            calculate(125., Some(&[1, 2, 3]), Binding::Valid(&v), None),
            0.
        );
        assert_eq!(
            calculate(125., Some(&[4, 3]), Binding::Valid(&v), None),
            125.
        );
    }
    #[test]
    fn null_class_is_wildcard_but_null_damage_type_bypasses_variance() {
        let v = Variance {
            include: &[Include {
                class: None,
                multiplier: 0.3,
            }],
            exclude: &[None],
        };
        assert_eq!(
            calculate(100., Some(&[1]), Binding::Valid(&v), None),
            30.000002
        );
        assert_eq!(calculate(100., None, Binding::Valid(&v), None), 100.);
    }
    #[test]
    fn explicit_override_and_invalid_override_do_not_fall_back() {
        let actor = Variance {
            include: &[],
            exclude: &[None],
        };
        let shield = Variance {
            include: &[],
            exclude: &[],
        };
        assert_eq!(
            calculate(
                125.,
                Some(&[1]),
                Binding::Valid(&actor),
                Some(Binding::Valid(&shield))
            ),
            125.
        );
        assert_eq!(
            calculate(
                125.,
                Some(&[1]),
                Binding::Valid(&actor),
                Some(Binding::Invalid)
            ),
            125.
        );
    }
    #[test]
    fn native_multiplication_does_not_clamp_or_filter_nonfinite_values() {
        let v = Variance {
            include: &[Include {
                class: None,
                multiplier: -2.,
            }],
            exclude: &[],
        };
        assert_eq!(calculate(125., Some(&[1]), Binding::Valid(&v), None), -250.);
        assert!(calculate(
            f32::INFINITY,
            Some(&[1]),
            Binding::Valid(&Variance {
                include: &[],
                exclude: &[None]
            }),
            None
        )
        .is_nan());
    }
}
