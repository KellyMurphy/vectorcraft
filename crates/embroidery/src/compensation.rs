//! Compensation fitted from a sew-out (E10.1b): the compensation sheet's swatches (sewn with no
//! compensation, [`crate::testsheet`] sections K–N) measured on the fabric give how far it draws
//! in along the stitches and spreads out across them, and from that the `pull`, `pullPercent`
//! and `push` that make the next design come out at its drawn size.
//!
//! The model is the generator's own: a satin `w` mm wide (or a fill row `w` mm long) is sewn
//! `pull + w × pullPercent / 100` wider, and a column (or a fill's rows) `push` shorter. Sewn
//! with none, a swatch shrinks along its stitches by `drawn − measured`, so a straight line
//! through each swatch's shrink against its drawn width gives `pull` (where it meets zero width)
//! and `pullPercent` (its slope); `push` is how much longer the swatches came out across their
//! stitches, on average. Each value is held to the range the stitch settings take.

use vectorcraft_doc::{SewSwatch, StitchSettings};

/// The compensation one set of swatches asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    /// mm, held to [`StitchSettings::PULL`].
    pub pull: f64,
    /// %, held to [`StitchSettings::PULL_PERCENT`].
    pub pull_percent: f64,
    /// mm, held to [`StitchSettings::PUSH`]; None when no swatch has a measured length.
    pub push: Option<f64>,
    /// How many swatches with a measured width the pull is fitted from.
    pub widths: usize,
    /// How many swatches with a measured length the push is the average of.
    pub lengths: usize,
    /// How far the measured widths lie from the fitted line, mm (root mean square): small when
    /// the swatches agree.
    pub spread: f64,
}

/// The fit of a sew-out: for each stitch direction measured, and for all of them together.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// Each angle (degrees counter-clockwise from the sheet's horizontal), in ascending order,
    /// with its fit.
    pub angles: Vec<(f64, Fitted)>,
    /// Every measured swatch together: the values a fabric profile takes until profiles keep one
    /// per direction.
    pub all: Fitted,
}

/// A drawn size and the same size measured, mm.
type Pair = (f64, f64);

/// A swatch's drawn and measured width (along its stitches) and length (across them), mm, when
/// both of a pair are there and make sense.
fn sizes(s: &SewSwatch) -> (Option<Pair>, Option<Pair>) {
    let pair = |drawn: Option<f64>, measured: Option<f64>| match (drawn, measured) {
        (Some(d), Some(m)) if d.is_finite() && m.is_finite() && d > 0.0 && m > 0.0 => Some((d, m)),
        _ => None,
    };
    (pair(s.drawn_width, s.measured_width), pair(s.drawn_length, s.measured_length))
}

/// The compensation `swatches` ask for, or None without at least one measured width.
fn fitted(swatches: &[&SewSwatch]) -> Option<Fitted> {
    let widths: Vec<(f64, f64)> = swatches.iter().filter_map(|s| sizes(s).0).map(|(d, m)| (d, d - m)).collect();
    let lengths: Vec<f64> = swatches.iter().filter_map(|s| sizes(s).1).map(|(d, m)| m - d).collect();
    if widths.is_empty() {
        return None;
    }
    let n = widths.len() as f64;
    let (mx, my) = (widths.iter().map(|w| w.0).sum::<f64>() / n, widths.iter().map(|w| w.1).sum::<f64>() / n);
    let sxx: f64 = widths.iter().map(|w| (w.0 - mx).powi(2)).sum();
    let sxy: f64 = widths.iter().map(|w| (w.0 - mx) * (w.1 - my)).sum();
    // One drawn width (or several the same) gives no slope: all of the shrink is fixed pull.
    let slope = if sxx > 1e-9 { sxy / sxx } else { 0.0 };
    let (mut slope, mut intercept) = (slope.max(0.0), my - slope.max(0.0) * mx);
    // A line that would need a negative pull: the share alone, through the average swatch.
    if intercept < 0.0 && slope > 0.0 {
        slope = (my / mx).max(0.0);
        intercept = 0.0;
    }
    let spread = (widths.iter().map(|w| (w.1 - (intercept + slope * w.0)).powi(2)).sum::<f64>() / n).sqrt();
    let clamp = |v: f64, r: std::ops::RangeInclusive<f64>| v.clamp(*r.start(), *r.end());
    let push = (!lengths.is_empty()).then(|| clamp(lengths.iter().sum::<f64>() / lengths.len() as f64, StitchSettings::PUSH));
    Some(Fitted {
        pull: round(clamp(intercept, StitchSettings::PULL), 100.0),
        pull_percent: round(clamp(slope * 100.0, StitchSettings::PULL_PERCENT), 10.0),
        push: push.map(|p| round(p, 100.0)),
        widths: widths.len(),
        lengths: lengths.len(),
        spread: round(spread, 100.0),
    })
}

fn round(v: f64, per: f64) -> f64 {
    (v * per).round() / per
}

/// The compensation the measured compensation swatches of a sew-out ask for, or None when no
/// swatch has a measured width yet.
pub fn fit(swatches: &[SewSwatch]) -> Option<Fit> {
    let measured: Vec<&SewSwatch> = swatches.iter().filter(|s| s.angle.is_some_and(f64::is_finite) && sizes(s).0.is_some()).collect();
    let all = fitted(&measured)?;
    let mut angles: Vec<f64> = measured.iter().filter_map(|s| s.angle).map(|a| round(a, 10.0)).collect();
    angles.sort_by(f64::total_cmp);
    angles.dedup();
    let angles = angles
        .into_iter()
        .filter_map(|a| {
            let these: Vec<&SewSwatch> = measured.iter().copied().filter(|s| s.angle.is_some_and(|b| round(b, 10.0) == a)).collect();
            fitted(&these).map(|f| (a, f))
        })
        .collect();
    Some(Fit { angles, all })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swatch(angle: f64, drawn: (f64, f64), measured: (Option<f64>, Option<f64>)) -> SewSwatch {
        SewSwatch {
            angle: Some(angle),
            drawn_width: Some(drawn.0),
            drawn_length: Some(drawn.1),
            measured_width: measured.0,
            measured_length: measured.1,
            ..Default::default()
        }
    }

    /// Swatches that shrank by 0.3 mm plus 5% of their width, and came out 0.2 mm longer, give
    /// back pull 0.3, 5% and push 0.2.
    #[test]
    fn a_fabric_that_draws_in_by_a_fixed_amount_and_a_share_is_fitted_exactly() {
        let shrink = |w: f64| 0.3 + 0.05 * w;
        let swatches: Vec<SewSwatch> =
            [2.0, 4.0, 6.0, 8.0, 18.0].iter().map(|w| swatch(0.0, (*w, 30.0), (Some(w - shrink(*w)), Some(30.2)))).collect();
        let fit = fit(&swatches).unwrap();
        assert_eq!((fit.all.pull, fit.all.pull_percent, fit.all.push), (0.3, 5.0, Some(0.2)));
        assert_eq!((fit.all.widths, fit.all.lengths), (5, 5));
        assert!(fit.all.spread < 0.01);
        assert_eq!(fit.angles.len(), 1);
    }

    /// Each angle gets its own fit, and the whole lies between them.
    #[test]
    fn each_direction_is_fitted_on_its_own() {
        let mut swatches = vec![];
        for (angle, pull) in [(0.0, 0.2), (90.0, 0.6)] {
            for w in [2.0, 4.0, 6.0, 8.0] {
                swatches.push(swatch(angle, (w, 30.0), (Some(w - pull), None)));
            }
        }
        let fit = fit(&swatches).unwrap();
        let by: Vec<(f64, f64, f64)> = fit.angles.iter().map(|(a, f)| (*a, f.pull, f.pull_percent)).collect();
        assert_eq!(by, [(0.0, 0.2, 0.0), (90.0, 0.6, 0.0)]);
        assert_eq!((fit.all.pull, fit.all.push), (0.4, None));
    }

    /// Nothing measured yet, a swatch measured bigger than drawn, and values past what the
    /// stitch settings take.
    #[test]
    fn odd_measurements_stay_in_range() {
        assert_eq!(fit(&[swatch(0.0, (4.0, 30.0), (None, Some(30.5)))]), None, "no width measured");
        assert_eq!(fit(&[]), None);
        let grew = fit(&[swatch(0.0, (4.0, 30.0), (Some(4.4), None))]).unwrap();
        assert_eq!((grew.all.pull, grew.all.pull_percent), (0.0, 0.0), "a swatch that came out wider needs no pull");
        let wild = fit(&[swatch(45.0, (2.0, 30.0), (Some(0.1), Some(40.0))), swatch(45.0, (8.0, 30.0), (Some(1.0), Some(40.0)))]).unwrap();
        assert!(wild.all.pull <= 3.0 && wild.all.pull_percent <= 30.0 && wild.all.push == Some(3.0), "{wild:?}");
        for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            assert_eq!(fit(&[swatch(0.0, (4.0, 30.0), (Some(bad), None))]), None, "{bad}");
        }
    }

    /// A fabric that shrinks narrow swatches more than wide ones: a fixed pull, no negative one.
    #[test]
    fn shrinking_less_when_wider_is_a_fixed_pull() {
        let fit = fit(&[swatch(0.0, (2.0, 30.0), (Some(1.0), None)), swatch(0.0, (8.0, 30.0), (Some(7.5), None))]).unwrap();
        assert!(fit.all.pull > 0.0 && fit.all.pull_percent == 0.0, "{fit:?}");
    }
}
