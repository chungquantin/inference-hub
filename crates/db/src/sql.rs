//! SQL fragments shared by every query that shows the same thing, so a price
//! or an organization is computed one way everywhere. Macros (not consts) so
//! they splice with `concat!` into static query strings.

/// The `organization_*` columns of [`ih_core::OrganizationRef`] for alias `$a`.
macro_rules! org_ref {
    ($a:literal) => {
        concat!(
            $a, ".slug AS organization_slug, ",
            $a, ".name AS organization_name, ",
            $a, ".logo_url AS organization_logo"
        )
    };
}

/// Blend of input and output price: `$w` input tokens per output token.
/// `$w` is a bind placeholder holding `BLEND_INPUT_WEIGHT`. Numeric math, so
/// callers can round it and 0.175 does not become 0.17500000000000002.
macro_rules! blended {
    ($input:literal, $output:literal, $w:literal) => {
        concat!(
            "(",
            $input,
            " * ",
            $w,
            "::numeric + ",
            $output,
            ") / (",
            $w,
            "::numeric + 1)"
        )
    };
}

/// Hourly instance price divided by its GPU count; NULL when there is none.
macro_rules! per_gpu_hour {
    ($price:literal, $count:literal) => {
        concat!($price, " / nullif(", $count, ", 0)")
    };
}

/// When an offering's price was last checked: confirmed, else first seen.
macro_rules! observed_at {
    ($a:literal) => {
        concat!("coalesce(", $a, ".confirmed_at, ", $a, ".observed_at)")
    };
}

/// An ILIKE pattern matching the words of `q` in order with anything between,
/// so `deepseek v3` finds `deepseek-v3` and `DeepSeek V3.1`. `%` and `\` in
/// the text match literally. `None` when `q` holds no words.
pub(crate) fn words_like(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '/' | ':'))
        .filter(|w| !w.is_empty())
        .map(|w| w.replace('\\', "\\\\").replace('%', "\\%"))
        .collect();
    (!words.is_empty()).then(|| format!("%{}%", words.join("%")))
}
