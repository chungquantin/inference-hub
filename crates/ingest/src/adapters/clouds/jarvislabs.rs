//! Jarvislabs: on-demand GPU prices from its public pricing page. The GPU table shows the
//! price of one GPU per row (the page lets the visitor pick 1, 2, 4 or 8 GPUs; the default
//! view is 1×), with vCPUs and RAM of that instance and a spot price beside it.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-jarvislabs",
    name: "Jarvislabs",
    organization: "jarvislabs",
    origin: "https://jarvislabs.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://jarvislabs.ai/pricing";

fn number(line: &str) -> Option<f64> {
    line.split_whitespace()
        .next()?
        .replace(',', "")
        .parse()
        .ok()
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let html = pages.first().ok_or("no page")?;
    let lines = text::html_lines(html);
    let head = lines
        .iter()
        .position(|l| l.ends_with("On-demand / hr Spot / hr"))
        .ok_or("no GPU table header")?
        + 1;
    let mut out: Vec<GpuListing> = Vec::new();
    let mut i = head;
    // Rows: name, VRAM, vCPUs, RAM, then one line with the on-demand price, the spot discount and the spot price. The page repeats
    // the rows as cards for small screens; the first repeated name ends the table.
    while i + 4 < lines.len() && lines[i].starts_with("NVIDIA") {
        let name = &lines[i];
        if out.iter().any(|l| &l.name == name) {
            break;
        }
        let usd = lines[i + 4]
            .starts_with('$')
            .then(|| text::first_dollar(&lines[i + 4]))
            .flatten()
            .ok_or_else(|| format!("no on-demand price for {name}: {:?}", lines[i + 4]))?;
        let prices = text::dollars(&lines[i + 4]);
        let spot = prices.get(1).copied();
        out.push(GpuListing {
            external_id: format!("{name}-1x"),
            name: name.clone(),
            gpu_type: gpu::normalize(name).unwrap_or_else(|| name.clone()),
            gpu_count: 1,
            price_per_hour: fx.to_usd(usd),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vram_gb": number(&lines[i + 1]),
                "vcpus": number(&lines[i + 2]),
                "ram_gb": number(&lines[i + 3]),
                "spot_per_hour": spot.map(|s| fx.to_usd(s)),
                "original": fx.original(usd),
            }),
        });
        i += 5;
    }
    if out.is_empty() {
        return Err("no GPU rows under the table header".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
<h2>GPUs</h2><p>Pick 1, 2, 4 or 8 GPUs to price an instance.</p>
<div role="row"><span>GPU</span><span>VRAM</span><span>vCPUs</span><span>RAM</span><span>On-demand / hr</span><span>Spot / hr</span></div>
<div role="row"><div><h3>NVIDIA H200 SXM</h3></div><div>141<span>GB</span></div><div>28</div><div>300<span>GB</span></div><span>$4.59</span><span><span>−<!-- -->50<!-- -->%</span><span>$2.29</span></span></div>
<div role="row"><div><h3>NVIDIA H100 SXM</h3></div><div>80<span>GB</span></div><div>16</div><div>200<span>GB</span></div><span>$3.49</span><span><span>−<!-- -->56<!-- -->%</span><span>$1.54</span></span></div>
<div role="row"><div><h3>NVIDIA A100 80GB</h3></div><div>80<span>GB</span></div><div>16</div><div>112<span>GB</span></div><span>$1.49</span><span><span>−<!-- -->40<!-- -->%</span><span>$0.89</span></span></div>
<div role="row"><div><h3>NVIDIA L4</h3></div><div>24<span>GB</span></div><div>32</div><div>124<span>GB</span></div><span>$0.44</span><span><span>−<!-- -->34<!-- -->%</span><span>$0.29</span></span></div>
<div class="card"><h3>NVIDIA H200 SXM</h3><span> 141 GB VRAM</span><span>On-demand / hr</span><span>$4.59</span></div>
"#;

    #[test]
    fn parses_rows_and_stops_at_the_card_copy() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4);
        let h100 = l.iter().find(|x| x.name == "NVIDIA H100 SXM").unwrap();
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.49);
        assert_eq!(h100.extra["spot_per_hour"], 1.54);
        assert_eq!(h100.extra["vcpus"], 16.0);
        assert_eq!(h100.extra["ram_gb"], 200.0);
        let a100 = l.iter().find(|x| x.name == "NVIDIA A100 80GB").unwrap();
        assert_eq!(a100.gpu_type, "A100 SXM");
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_on_unreadable_price_or_missing_table() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
        let bad = FIXTURE.replace("$4.59</span><span><span>", "Ask</span><span><span>");
        assert!(parse(&[bad], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_JARVISLABS").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
