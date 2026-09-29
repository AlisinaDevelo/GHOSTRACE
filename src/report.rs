//! A self-contained, offline HTML timeline for a person to read.
//!
//! The report is a plaintext disclosure generated only on request. It uses no
//! scripts, fonts, images, or links, and its Content-Security-Policy forbids
//! loading anything, so opening it never contacts a network. Every statement
//! comes from the claim grammar: an event the grammar will not describe is
//! shown as an abstention, a gap is shown as its own unobserved interval, and
//! the header never presents the journal as complete coverage.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::{
    claims::{render_claim, ClaimLocale},
    model::{EventEnvelope, EventPayload, Evidence},
};

/// Shown at the top of every report.
pub const REPORT_PLAINTEXT_NOTICE: &str = "This file is an unencrypted copy of part of your \
GHOSTRACE journal. Anyone who can open it can read it, and deleting journal records does not \
delete this copy.";

/// Render `events` (in any order) as one HTML document.
pub fn render_timeline_html(events: &[EventEnvelope], generated_at: DateTime<Utc>) -> String {
    let mut ordered = events.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|event| (event.observed_at, event.event_id));

    let mut by_source = BTreeMap::<String, usize>::new();
    let (mut gaps, mut abstentions) = (0usize, 0usize);
    let mut rows = String::new();
    let mut day = None;
    for event in &ordered {
        *by_source.entry(event.source.to_string()).or_default() += 1;
        let date = event.observed_at.date_naive();
        if day != Some(date) {
            day = Some(date);
            rows.push_str(&format!(
                "<li class=\"day\"><h2>{}</h2></li>\n",
                escape(&date.format("%A %-d %B %Y").to_string())
            ));
        }
        let time = escape(&event.observed_at.format("%H:%M:%S UTC").to_string());
        let source = escape(&event.source.to_string());
        if let EventPayload::Gap(gap) = &event.payload {
            gaps += 1;
            rows.push_str(&format!(
                "<li class=\"row gap\"><span class=\"time\">{time}</span>\
                 <span class=\"badge gap\">not observed</span>\
                 <p><strong>Gap in {}:</strong> {}{}. What this gap covers is unknown; no \
                 statement fills it.</p></li>\n",
                escape(&gap.source.to_string()),
                escape(gap.reason_code.as_str()),
                match gap.dropped_count {
                    0 => String::new(),
                    count => format!(" ({count} event(s) missing)"),
                },
            ));
            continue;
        }
        match render_claim(event, ClaimLocale::En, false) {
            Ok(claim) => {
                let (class, label) = evidence_class(claim.evidence);
                rows.push_str(&format!(
                    "<li class=\"row {class}\"><span class=\"time\">{time}</span>\
                     <span class=\"badge {class}\">{label}</span>\
                     <span class=\"source\">{source}</span><p>{}</p></li>\n",
                    escape(&claim.text)
                ));
            }
            Err(_) => {
                abstentions += 1;
                rows.push_str(&format!(
                    "<li class=\"row abstain\"><span class=\"time\">{time}</span>\
                     <span class=\"badge abstain\">no claim</span>\
                     <span class=\"source\">{source}</span><p>A {} event was recorded, but its \
                     evidence does not support a statement, so GHOSTRACE abstains.</p></li>\n",
                    escape(&event.kind.to_string())
                ));
            }
        }
    }

    let coverage = if gaps > 0 {
        format!(
            "<p class=\"coverage incomplete\"><strong>Coverage is incomplete.</strong> {gaps} \
             gap(s) were recorded; activity inside them is unknown.</p>"
        )
    } else {
        "<p class=\"coverage\"><strong>No gaps were recorded.</strong> That does not mean \
         everything was observed: only the sources you enabled were recorded, and only while \
         they ran.</p>"
            .to_owned()
    };
    let sources = by_source
        .iter()
        .map(|(source, count)| format!("<li>{}: {count}</li>", escape(source)))
        .collect::<String>();
    let empty = if ordered.is_empty() {
        "<li class=\"row\"><p>The journal has no events.</p></li>"
    } else {
        ""
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
         style-src 'unsafe-inline'\">\n<meta name=\"referrer\" content=\"no-referrer\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>GHOSTRACE timeline</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n\
         <h1>GHOSTRACE timeline</h1>\n<p class=\"notice\">{}</p>\n\
         <p class=\"meta\">Generated {} from {} event(s). {} abstention(s).</p>\n{coverage}\n\
         <ul class=\"sources\">{sources}</ul>\n\
         <p class=\"legend\"><span class=\"badge direct\">direct</span> observed by the source \
         <span class=\"badge contextual\">contextual</span> nearby, not causal \
         <span class=\"badge inferred\">inferred</span> derived, not observed \
         <span class=\"badge gap\">not observed</span> a gap \
         <span class=\"badge abstain\">no claim</span> abstained</p>\n\
         <ol class=\"timeline\">\n{empty}{rows}</ol>\n</main>\n</body>\n</html>\n",
        escape(REPORT_PLAINTEXT_NOTICE),
        escape(&generated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
        ordered.len(),
        abstentions,
    )
}

fn evidence_class(evidence: Evidence) -> (&'static str, &'static str) {
    match evidence {
        Evidence::Direct => ("direct", "direct"),
        Evidence::Contextual => ("contextual", "contextual"),
        Evidence::Inferred => ("inferred", "inferred"),
        Evidence::Unknown => ("unknown", "unknown"),
    }
}

/// Escape text for an HTML element or attribute value.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

const STYLE: &str = "
:root{--bg:#fbfaf7;--fg:#1d1d1f;--muted:#5d5d63;--line:#d9d6cf;--direct:#1f6f43;
--contextual:#8a5a00;--inferred:#5b3fa3;--gap:#a1261d;--card:#fff}
@media (prefers-color-scheme:dark){:root{--bg:#161618;--fg:#ececef;--muted:#a3a3ab;
--line:#3a3a40;--direct:#6fcf97;--contextual:#f2c46d;--inferred:#b9a3f5;--gap:#ff8a80;
--card:#1f1f23}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);
font:15px/1.5 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}
main{max-width:860px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:24px;margin:0 0 12px}
h2{font-size:14px;color:var(--muted);margin:24px 0 8px;font-weight:600}
.notice{border-left:4px solid var(--gap);padding:8px 12px;background:var(--card)}
.meta,.legend{color:var(--muted);font-size:13px}
.coverage{padding:8px 12px;border:1px solid var(--line);background:var(--card)}
.coverage.incomplete{border-color:var(--gap)}
.sources{display:flex;flex-wrap:wrap;gap:12px;padding:0;list-style:none;color:var(--muted);
font-size:13px}
.timeline{list-style:none;padding:0;margin:0}
.day{list-style:none}
.row{display:grid;grid-template-columns:110px auto 1fr;gap:4px 12px;align-items:baseline;
padding:10px 12px;margin:6px 0;background:var(--card);border:1px solid var(--line);
border-left-width:4px;overflow-wrap:anywhere}
.row p{grid-column:1/-1;margin:0}
.time{font-variant-numeric:tabular-nums;color:var(--muted);font-size:13px}
.source{color:var(--muted);font-size:13px}
.badge{display:inline-block;font-size:12px;padding:0 6px;border:1px solid currentColor}
.direct{border-left-color:var(--direct)}.badge.direct{color:var(--direct);border-style:solid}
.contextual{border-left-color:var(--contextual)}
.badge.contextual{color:var(--contextual);border-style:dashed}
.inferred{border-left-color:var(--inferred)}.badge.inferred{color:var(--inferred);border-style:dotted}
.unknown{border-left-color:var(--muted)}.badge.unknown{color:var(--muted)}
.row.gap{border-left-color:var(--gap);border-style:dashed;
background:repeating-linear-gradient(135deg,var(--card) 0 8px,transparent 8px 16px)}
.badge.gap{color:var(--gap);font-weight:600}
.row.abstain{border-left-color:var(--muted);border-left-style:dotted}
.badge.abstain{color:var(--muted);border-style:dotted}
@media (max-width:520px){.row{grid-template-columns:1fr auto}.source{grid-column:1/-1}}
";

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escape_neutralizes_markup() {
        assert_eq!(
            escape("<script>alert('x')</script> & \"q\""),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;q&quot;"
        );
    }
}
