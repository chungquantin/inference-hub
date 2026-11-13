//! Mithril: instance prices from its public pricing page. The compute table lists GPU type,
//! GPU count, vCPUs, RAM, disk and a "Spot (7-day avg)" price per instance-hour: Mithril sets
//! prices algorithmically from supply and demand and the page shows the average of the last
//! seven days, not a fixed on-demand list price. The parser is complete but the cloud stays
//! disabled so market averages are not shown next to on-demand prices (see the report).

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-mithril",
    name: "Mithril",
    organization: "mithril",
    origin: "https://mithril.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // The only price on the page is a 7-day spot average, not an on-demand price.
    enabled: false,
    parse,
};

const URL: &str = "https://mithril.ai/pricing";

fn int(line: &str) -> Option<i64> {
    line.replace(',', "").trim().parse().ok()
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let html = pages.first().ok_or("no page")?;
    let lines = text::html_lines(html);
    let head = lines
        .iter()
        .position(|l| l == "Spot (7-day avg)")
        .ok_or("no compute table header")?
        + 1;
    let mut out: Vec<GpuListing> = Vec::new();
    let mut i = head;
    // Rows of nine lines: type, GPUs, vCPUs, RAM, "GB", disk, "GB", price, "instance/hr".
    while i + 8 < lines.len() && int(&lines[i + 1]).is_some() && int(&lines[i + 2]).is_some() {
        let r = &lines[i..i + 9];
        i += 9;
        let (name, count, vcpus) = (&r[0], int(&r[1]), int(&r[2]));
        let (ram, disk) = (int(&r[3]), int(&r[5]));
        let (Some(count), Some(vcpus), Some(ram)) = (count, vcpus, ram) else {
            return Err(format!("unreadable specs for {name}"));
        };
        // A row without a stated price is not a configuration to list.
        let Some(usd) = r[7]
            .starts_with('$')
            .then(|| text::first_dollar(&r[7]))
            .flatten()
        else {
            continue;
        };
        out.push(GpuListing {
            // The same GPU and count is offered on differently sized hosts.
            external_id: format!("{name}-{count}x-{vcpus}vcpu-{ram}gb"),
            name: format!("{count}× {name}"),
            gpu_type: gpu::normalize(name).unwrap_or_else(|| name.clone()),
            gpu_count: i16::try_from(count).map_err(|_| format!("GPU count of {name}"))?,
            price_per_hour: fx.to_usd(usd),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vcpus": vcpus,
                "ram_gb": ram,
                "disk_gb": disk,
                "price_basis": "spot, 7-day average, per instance-hour",
                "original": fx.original(usd),
            }),
        });
    }
    if out.is_empty() {
        return Err("no priced rows under the compute table header".into());
    }
    let mut ids: Vec<_> = out.iter().map(|l| l.external_id.as_str()).collect();
    ids.sort();
    if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("duplicate configuration {}", w[0]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: [&str; 9]) -> String {
        cells
            .iter()
            .map(|c| format!("<div><p class=\"framer-text\">{c}</p></div>"))
            .collect()
    }

    fn fixture() -> String {
        let head = [
            "Instance type",
            "GPUs",
            "vCPUs",
            "RAM",
            "Disk",
            "Spot (7-day avg)",
        ]
        .iter()
        .map(|c| format!("<div><p>{c}</p></div>"))
        .collect::<String>();
        let rows = [
            row([
                "H200 SXM5",
                "1",
                "20",
                "227",
                "GB",
                "3,300",
                "GB",
                "$3.93",
                "instance/hr",
            ]),
            row([
                "H200 SXM5",
                "1",
                "20",
                "200",
                "GB",
                "1,500",
                "GB",
                "$2.15",
                "instance/hr",
            ]),
            row([
                "H100 SXM5",
                "8",
                "168",
                "1,814",
                "GB",
                "12,500",
                "GB",
                "$21.00",
                "instance/hr",
            ]),
            row([
                "B200 SXM6",
                "8",
                "168",
                "1,800",
                "GB",
                "12,500",
                "GB",
                "Contact us",
                "instance/hr",
            ]),
        ]
        .concat();
        format!("{head}{rows}<p>Mithril's omnicloud platform sets compute prices.</p>")
    }

    #[test]
    fn parses_rows_and_skips_unpriced() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3);
        let h100 = l.iter().find(|x| x.gpu_type == "H100 SXM").unwrap();
        assert_eq!(h100.gpu_count, 8);
        assert_eq!(h100.price_per_hour, 21.0);
        assert_eq!(h100.extra["ram_gb"], 1814);
        let h200: Vec<_> = l.iter().filter(|x| x.gpu_type == "H200 SXM").collect();
        assert_eq!(h200.len(), 2);
        assert_eq!(h200[0].price_per_hour, 3.93);
        assert!(l.iter().all(|x| x.gpu_type != "B200"));
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_the_table() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_MITHRIL").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
