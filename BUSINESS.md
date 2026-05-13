# UBench Business Strategy

> **Mission**: Make UBench the global standard for honest portable storage benchmarking, so consumers can finally tell good drives from bad ones — and good small brands can compete with marketing-heavy giants on data, not slogans.

---

## 1. The Market Pain (Why This Will Work)

### Consumer side
- USB drives and portable SSDs have **no trusted independent ranking**. Amazon reviews are gamed; vendor specs are inflated; AS SSD/CDM screenshots from sellers measure RAM cache, not flash.
- 80%+ of cheap-USB-market drives are **fake-capacity scams** that show fine in standard benchmarks because the test region (1 GB by default) fits within the real capacity.
- "Why is my brand-new 64GB USB so slow?" — answered by no one.

### Manufacturer side (the asymmetry that creates opportunity)
- **Big brands** (Sandisk, Samsung, Kingston) win because of marketing, not always product quality.
- **Small/factory-direct brands** in Shenzhen, Guangzhou, etc., often have *better* flash chips (real Hynix/Micron NAND) but no way to prove it. They cannot reach consumers because reviews are saturated by big brands.
- A neutral, public, signed scoreboard that small brands can submit to = **leveling the playing field**.

---

## 2. The Three-Layer Plan

### Layer 1 — Open-source CLI tool (DONE)
- MIT-licensed, runs on Windows/macOS, single binary
- Anyone can download and run on their own drives
- Output: signed JSON report (SHA-256), grade A–F, UBench Score 0–100
- Compatible with AS SSD numbers (so users can verify their merchant's screenshots)

### Layer 2 — Public scoreboard (`udiskbench.org`)
- Users upload signed JSON reports
- Aggregated by `vendor + model + capacity + interface`
- Median UBS per drive model, sample size visible
- Filter: "Best USB-A under $20", "Best portable SSD for video editing", "Avoid these fakes"
- **No editorial influence — purely data-driven**

### Layer 3 — Vendor program (revenue)
Three tiers, all opt-in:

| Tier | Name | Price | What vendor gets | What we promise |
|---|---|---:|---|---|
| 1 | **Verified** | Free | Green checkmark next to model on scoreboard | We send a free engineering sample, run UBench Full 5x, publish raw signed reports |
| 2 | **Sponsor** | $500/yr | Logo on scoreboard sidebar; sponsored slot in "Best in class" lists (clearly labeled "Sponsored") | Quarterly re-test for free; no review censorship |
| 3 | **Lab certified** | $5,000 one-time | Independent multi-unit testing (10 drives, 30-day burn-in, sustained workload), "UBench Lab Certified" badge they can put on packaging | Public test report, can withdraw if drive fails certification |

**Key rule: We never refuse to publish bad scores.** If a sponsor's drive scores poorly, that score still appears. They can withdraw sponsorship but not the data. This is the only way to maintain consumer trust → which is the only way to maintain vendor demand for the certification.

---

## 3. Why "带货" (Influencer commerce) is the natural fit

The Chinese consumer-tech YouTube/Bilibili scene already does this with phones (鲁大师 / Antutu scores in every review). USB drives have no equivalent.

A typical content creator workflow:
1. Buy 5 USB drives from JD/Taobao
2. Run `ubench bench` on each, screen-record
3. Show side-by-side UBS scores
4. Drop affiliate links to the winners

For this to take off we need:
- Branded, screenshot-friendly output (current text table is OK, but a one-click HTML/image export would be better)
- A "share this result" URL on `udiskbench.org` that creators can drop in video descriptions
- Affiliate-link integration: when clicking "Buy on JD" we get a small commission, kicked back to drive future development

---

## 4. Defensive moats (why we won't get displaced)

1. **The standard, not just a tool** — `SPEC.md` is open. Any third party can implement a UBench-v1.0-compliant tester. Every implementation reports comparable scores. We become the W3C of USB benchmarking, not the Microsoft.
2. **Signed reports** — SHA-256 over canonical JSON. A vendor can't fake a high score without rewriting the spec — and rewritten specs aren't UBench.
3. **Network effect on the scoreboard** — As more reports come in, the median per drive model becomes statistically reliable. Late entrants can't easily replicate years of crowd-sourced data.
4. **Honesty branding** — Every other benchmark lies (RAM cache effects). UBench's whole identity is "the one that doesn't." This is hard to copy without admitting your own past dishonesty.

---

## 5. Roadmap

### v1.0 (now) — The core tool
- ✅ Direct I/O (the foundational honesty rule)
- ✅ UBench Score 0–100 + Grade A–F
- ✅ AS SSD compatibility mode
- ✅ SHA-256 signed JSON
- ⏳ USB interface detection (USB 2.0 vs 3.0 vs 3.2 makes a 30x speed difference)

### v1.1 — Device classification + better calibration
- Detect device type: **USB flash drive / portable SSD / internal SSD / HDD**
- Apply different scoring curves per device class (an SSD scoring 90 should not equate to a USB scoring 90)
- Calibrate against 30+ real drives across price tiers

### v1.2 — Scoreboard launch (`udiskbench.org`)
- Static-first (Cloudflare Pages): JSON dump per model
- Submission via signed PR (early users) or web upload (later)
- Filter / search / share URLs

### v2.0 — The vendor program
- Tier 1/2/3 onboarding
- Lab kit (we ship a USB hub + reference drives to a few independent reviewers globally)
- Affiliate integration

---

## 6. What I (the maintainer) need from you (the user) to make this happen

**Now:**
- [ ] Decide: is `udiskbench.org` the domain, or something else? (`uscore.org`, `usbench.io`...)
- [ ] Get a few more drives for calibration: 1 Sandisk Extreme Pro, 1 Samsung Bar Plus, 1 generic Shenzhen-made — total ~$50

**In a month:**
- [ ] Reach out to 3–5 small Chinese USB factories. Offer Tier 1 for free in exchange for a public statement. Nothing converts skeptics like "Brand X's CEO endorses our standard."
- [ ] Find 2 Bilibili/YouTube tech reviewers willing to use UBench in their next USB roundup video. Offer them early access + co-branded badge.

**By Q3 2026:**
- [ ] Publish v1.1 with device classification
- [ ] Launch scoreboard with at least 50 drive models
- [ ] First paying Sponsor tier sign-up
