//! GPU names differ by cloud (`NVIDIA H100 80GB HBM3`, `H100 SXM`, `h100`/`sxm5`,
//! `NVIDIA HGX H100`). Comparisons need one name per GPU, so every neocloud
//! adapter maps its names through [`normalize`].

/// The directory's name for a GPU, such as `H100 SXM` or `L40S`, or `None` when
/// the name is not recognized. Unrecognized GPUs are kept as listings but not
/// compared, rather than compared under a wrong name.
pub fn normalize(raw: &str) -> Option<String> {
    let s = raw
        .to_ascii_uppercase()
        .replace(['-', '_'], " ")
        .replace("NVIDIA", "")
        .replace("GEFORCE", "")
        .replace("HGX", "")
        .replace("TENSOR CORE GPU", "");
    let words: Vec<&str> = s.split_whitespace().collect();
    let has = |w: &str| words.iter().any(|x| *x == w || x.starts_with(w));
    let first = *words.first()?;

    // Form factor of data-center Hopper and Blackwell parts.
    let form = if has("NVL") {
        " NVL"
    } else if has("PCIE") {
        " PCIe"
    } else {
        " SXM"
    };
    let name = match first {
        "H100" => format!("H100{form}"),
        "H200" => format!("H200{form}"),
        "B200" | "B300" => {
            if has("NVL") {
                return None; // rack-scale variants are not single-node configurations
            }
            first.to_owned()
        }
        "GH200" => "GH200".to_owned(),
        "GB200" | "GB300" => format!("{first} NVL72"),
        "A100" => {
            let mem = if has("40GB") || words.contains(&"40") {
                " 40GB"
            } else {
                ""
            };
            let form = if has("PCIE") { " PCIe" } else { " SXM" };
            format!("A100{form}{mem}")
        }
        "L40S" | "L40" | "L4" | "A40" | "A10" | "A10G" | "T4" | "V100" => first.to_owned(),
        "MI300X" | "MI325X" | "MI355X" => format!("AMD {first}"),
        "AMD" => match words.get(1).copied() {
            Some("INSTINCT") => format!("AMD {}", words.get(2)?),
            Some(model) => format!("AMD {model}"),
            None => return None,
        },
        "RTX" => {
            let rest = words[1..].join(" ");
            format!("RTX {}", title_case(&rest))
        }
        _ => return None,
    };
    Some(name)
}

/// `PRO 6000 BLACKWELL` → `PRO 6000 Blackwell`, keeping model numbers and acronyms.
fn title_case(s: &str) -> String {
    const ACRONYMS: [&str; 4] = ["PRO", "SE", "SFF", "TI"];
    s.split_whitespace()
        .map(|w| {
            if ACRONYMS.contains(&w) || w.chars().any(|c| c.is_ascii_digit()) {
                w.to_owned()
            } else {
                let mut out = w[..1].to_owned();
                out.push_str(&w[1..].to_ascii_lowercase());
                out
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn maps_each_clouds_names_to_one_name() {
        for (raw, expected) in [
            // RunPod ids
            ("NVIDIA H100 80GB HBM3", "H100 SXM"),
            ("NVIDIA H100 PCIe", "H100 PCIe"),
            ("NVIDIA H100 NVL", "H100 NVL"),
            ("NVIDIA H200", "H200 SXM"),
            ("NVIDIA H200 NVL", "H200 NVL"),
            ("NVIDIA B200", "B200"),
            ("NVIDIA A100-SXM4-80GB", "A100 SXM"),
            ("NVIDIA A100-SXM4-40GB", "A100 SXM 40GB"),
            ("NVIDIA A100 80GB PCIe", "A100 PCIe"),
            ("NVIDIA L40S", "L40S"),
            ("AMD Instinct MI300X OAM", "AMD MI300X"),
            ("NVIDIA GeForce RTX 4090", "RTX 4090"),
            ("NVIDIA RTX 6000 Ada Generation", "RTX 6000 Ada Generation"),
            // Hyperbolic gpuType + form factor
            ("h100 sxm5", "H100 SXM"),
            ("h200 sxm5", "H200 SXM"),
            // CoreWeave names
            ("NVIDIA HGX H100", "H100 SXM"),
            ("NVIDIA HGX H200", "H200 SXM"),
            ("NVIDIA HGX B200", "B200"),
            ("NVIDIA GB200 NVL72", "GB200 NVL72"),
            ("NVIDIA GH200", "GH200"),
            ("NVIDIA A100", "A100 SXM"),
            // The name the curated data already uses
            ("H100 SXM", "H100 SXM"),
        ] {
            assert_eq!(normalize(raw).as_deref(), Some(expected), "{raw}");
        }
    }

    #[test]
    fn unknown_names_are_not_guessed() {
        for raw in ["", "NVIDIA", "Quantum Widget 9000", "TPU v5e"] {
            assert_eq!(normalize(raw), None, "{raw}");
        }
    }
}
