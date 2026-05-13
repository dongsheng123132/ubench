pub mod output;

use crate::db::Database;
use crate::disk::{detect, info};
use crate::types::*;
use crate::STOP_FLAG;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Table};
use indicatif::{ProgressBar, ProgressStyle};
use output::{print_error, print_success, OutputMode};
use std::sync::atomic::Ordering;

#[derive(Parser)]
#[command(name = "ubench", version, about = "UBench - 行业标准 U盘跑分工具 (绕过缓存的真实速度测量)")]
pub struct Cli {
    /// 以 JSON 格式输出（适合 AI/脚本调用）
    #[arg(long, global = true)]
    json: bool,

    /// 数据库路径
    #[arg(long, global = true, default_value = "ubench_reports.db")]
    db: String,

    /// 详细日志
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 列出 USB 设备
    List,
    /// 设备详情
    Info {
        /// 驱动器路径（如 E: 或 /dev/disk2）
        drive: String,
    },
    /// 运行测试
    Test {
        /// 挂载点（如 E:\ 或 /Volumes/USB）
        mount: String,
        /// 容量测试
        #[arg(long)]
        capacity: bool,
        /// 速度测试
        #[arg(long)]
        speed: bool,
        /// 坏块测试
        #[arg(long)]
        badblock: bool,
        /// 热稳定性测试（指定持续秒数）
        #[arg(long)]
        thermal: Option<u64>,
        /// 运行所有测试
        #[arg(long)]
        all: bool,
        /// 不保存到数据库
        #[arg(long)]
        no_save: bool,
        /// 导出 HTML 报告
        #[arg(long)]
        export_html: Option<String>,
        /// 速度测试大小（MB）
        #[arg(long, default_value = "1024")]
        test_size_mb: u64,
    },
    /// 报告管理
    Report {
        #[command(subcommand)]
        action: ReportAction,
    },
    /// 跑分 (UBench v1.0 行业标准 — 绕过 OS 缓存的诚实测速)
    Bench {
        /// 挂载点（如 E:\, F:\, X:\, /Volumes/USB）
        mount: String,
        /// 快速模式 (~1 分钟,4 项核心测试)
        #[arg(long)]
        quick: bool,
        /// 完整模式 (~10 分钟,8 项含 4K-64Thrd / SLC 缓存衰减 / 冷启动 / U-Claw 实景)
        #[arg(long)]
        full: bool,
        /// AS SSD Benchmark 兼容模式 (~3 分钟,6 项与 AS SSD 完全对齐,可直接对比商家截图)
        #[arg(long)]
        asssd: bool,
        /// 输出 JSON 报告文件路径 (含签名,可上传至 udiskbench.org)
        #[arg(long)]
        out: Option<String>,
        /// 导出 HTML 跑分报告（含可视化图表，适合截图分享）
        #[arg(long)]
        export_html: Option<String>,
    },
}

#[derive(Subcommand)]
enum ReportAction {
    /// 列出历史报告
    List,
    /// 查看报告详情
    Show {
        /// 报告 ID
        id: String,
    },
    /// 导出 HTML 报告
    Export {
        /// 报告 ID
        id: String,
        /// 输出 HTML 文件路径
        #[arg(long)]
        html: String,
    },
    /// 删除报告
    Delete {
        /// 报告 ID
        id: String,
    },
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mode = if cli.json {
        OutputMode::Json
    } else {
        OutputMode::Human
    };

    match cli.command {
        Commands::List => cmd_list(mode).await,
        Commands::Info { drive } => cmd_info(mode, &drive).await,
        Commands::Test {
            mount,
            capacity,
            speed,
            badblock,
            thermal,
            all,
            no_save,
            export_html,
            test_size_mb,
        } => {
            cmd_test(
                mode,
                &cli.db,
                &mount,
                capacity || all,
                speed || all,
                badblock || all,
                if all { Some(120) } else { thermal },
                no_save,
                export_html.as_deref(),
                test_size_mb,
            )
            .await
        }
        Commands::Report { action } => match action {
            ReportAction::List => cmd_report_list(mode, &cli.db),
            ReportAction::Show { id } => cmd_report_show(mode, &cli.db, &id),
            ReportAction::Export { id, html } => cmd_report_export(mode, &cli.db, &id, &html),
            ReportAction::Delete { id } => cmd_report_delete(mode, &cli.db, &id),
        },
        Commands::Bench { mount, quick, full, asssd, out, export_html } => {
            let mode_count = (quick as u8) + (full as u8) + (asssd as u8);
            if mode_count > 1 {
                return Err("--quick / --full / --asssd 只能三选一".into());
            }
            let profile = if full {
                crate::bench::BenchProfile::Full
            } else if asssd {
                crate::bench::BenchProfile::AsSsdCompat
            } else {
                crate::bench::BenchProfile::Quick // default
            };
            cmd_bench(mode, &mount, profile, out.as_deref(), export_html.as_deref()).await
        }
    }
}

async fn cmd_list(mode: OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    let drives = tokio::task::spawn_blocking(detect::list_usb_drives)
        .await?
        .map_err(|e| e)?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &drives);
        }
        OutputMode::Human => {
            if drives.is_empty() {
                println!("No USB drives found.");
                return Ok(());
            }
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec![
                "Name", "Path", "Mount", "Capacity", "Free", "FS", "Serial",
            ]);
            for d in &drives {
                table.add_row(vec![
                    &d.name,
                    &d.path,
                    &d.mount_point,
                    &info::format_capacity(d.capacity_bytes),
                    &info::format_capacity(d.free_bytes),
                    &d.file_system,
                    &d.serial,
                ]);
            }
            println!("{table}");
        }
    }
    Ok(())
}

async fn cmd_info(mode: OutputMode, drive: &str) -> Result<(), Box<dyn std::error::Error>> {
    let drive_str = drive.to_string();
    let drives = tokio::task::spawn_blocking(detect::list_usb_drives)
        .await?
        .map_err(|e| e)?;

    let found = drives
        .into_iter()
        .find(|d| d.path == drive_str || d.mount_point == drive_str)
        .ok_or_else(|| format!("Drive not found: {}", drive_str))?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &found);
        }
        OutputMode::Human => {
            println!("{}", info::drive_summary(&found));
            println!("  Path:       {}", found.path);
            println!("  Mount:      {}", found.mount_point);
            println!(
                "  Capacity:   {}",
                info::format_capacity(found.capacity_bytes)
            );
            println!("  Free:       {}", info::format_capacity(found.free_bytes));
            println!("  FS:         {}", found.file_system);
            println!("  Serial:     {}", found.serial);
            println!("  Removable:  {}", found.is_removable);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_test(
    mode: OutputMode,
    db_path: &str,
    mount: &str,
    test_capacity: bool,
    test_speed: bool,
    test_badblock: bool,
    test_thermal: Option<u64>,
    no_save: bool,
    export_html: Option<&str>,
    test_size_mb: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if !test_capacity && !test_speed && !test_badblock && test_thermal.is_none() {
        let msg = "No test selected. Use --capacity, --speed, --badblock, --thermal SECS, or --all";
        print_error(mode, msg);
        return Err(msg.into());
    }

    STOP_FLAG.store(false, Ordering::Relaxed);

    let start_time = std::time::Instant::now();

    // Get drive info
    let mount_str = mount.to_string();
    let drives = tokio::task::spawn_blocking(detect::list_usb_drives)
        .await?
        .map_err(|e| e)?;

    let drive_info = drives
        .into_iter()
        .find(|d| d.path == mount_str || d.mount_point == mount_str)
        .ok_or_else(|| format!("Drive not found at mount point: {}", mount_str))?;

    let drive_name = drive_info.name.clone();
    let claimed_capacity = drive_info.capacity_bytes;
    let mount_point = drive_info.mount_point.clone();

    if let OutputMode::Human = mode {
        println!(
            "Testing: {} ({})",
            drive_name,
            info::format_capacity(claimed_capacity)
        );
    }

    let mut real_capacity: Option<u64> = None;
    let mut seq_read_speed: Option<f64> = None;
    let mut seq_write_speed: Option<f64> = None;
    let mut random_read_iops: Option<f64> = None;
    let mut random_write_iops: Option<f64> = None;
    let mut speed_stability: Option<f64> = None;
    let mut speed_samples: Vec<SpeedSample> = Vec::new();
    let mut bad_block_count: Option<u64> = None;
    let mut total_blocks: Option<u64> = None;
    let mut bad_block_positions: Vec<u64> = Vec::new();
    let mut speed_drops: Option<u32> = None;

    // Capacity test
    if test_capacity {
        let mount_c = mount_point.clone();
        let pb = make_progress_bar(mode, "Capacity test");

        let result = tokio::task::spawn_blocking(move || {
            crate::test::capacity::run_capacity_test(&mount_c, claimed_capacity, |progress, msg, _spd| {
                eprint_progress(&pb, progress, msg);
            })
        })
        .await?;

        match result {
            Ok(cap) => {
                real_capacity = Some(cap.real_bytes);
                total_blocks = Some(cap.total_blocks);
                bad_block_positions.extend(cap.bad_blocks.iter());
                if let OutputMode::Human = mode {
                    eprintln!(
                        "  Capacity: {} real / {} claimed",
                        info::format_capacity(cap.real_bytes),
                        info::format_capacity(claimed_capacity)
                    );
                }
            }
            Err(e) => {
                eprintln!("  Capacity test error: {}", e);
            }
        }
    }

    // Speed test
    if test_speed && !STOP_FLAG.load(Ordering::Relaxed) {
        let mount_c = mount_point.clone();
        let pb = make_progress_bar(mode, "Speed test");

        let result = tokio::task::spawn_blocking(move || {
            crate::test::speed::run_speed_test(&mount_c, test_size_mb, |progress, msg, _spd| {
                eprint_progress(&pb, progress, msg);
            })
        })
        .await?;

        match result {
            Ok(spd) => {
                seq_write_speed = Some(spd.seq_write_speed);
                seq_read_speed = Some(spd.seq_read_speed);
                random_read_iops = Some(spd.random_read_iops);
                random_write_iops = Some(spd.random_write_iops);
                speed_stability = Some(spd.stability);
                speed_drops = Some(spd.speed_drops);

                let max_len = spd.write_samples.len().max(spd.read_samples.len());
                for i in 0..max_len {
                    let ws = spd.write_samples.get(i).map(|s| s.speed_mbps).unwrap_or(0.0);
                    let rs = spd.read_samples.get(i).map(|s| s.speed_mbps).unwrap_or(0.0);
                    let offset = spd
                        .write_samples
                        .get(i)
                        .or_else(|| spd.read_samples.get(i))
                        .map(|s| s.offset_mb)
                        .unwrap_or(0);
                    speed_samples.push(SpeedSample {
                        offset_mb: offset,
                        write_speed: ws,
                        read_speed: rs,
                    });
                }

                if let OutputMode::Human = mode {
                    eprintln!(
                        "  Seq Write: {:.1} MB/s | Seq Read: {:.1} MB/s",
                        spd.seq_write_speed, spd.seq_read_speed
                    );
                    eprintln!(
                        "  4K Write: {:.0} IOPS | 4K Read: {:.0} IOPS",
                        spd.random_write_iops, spd.random_read_iops
                    );
                }
            }
            Err(e) => {
                eprintln!("  Speed test error: {}", e);
            }
        }
    }

    // Bad block test
    if test_badblock && !STOP_FLAG.load(Ordering::Relaxed) {
        let mount_c = mount_point.clone();
        let pb = make_progress_bar(mode, "Bad block test");

        let result = tokio::task::spawn_blocking(move || {
            crate::test::badblock::run_badblock_test(&mount_c, |progress, msg, _spd| {
                eprint_progress(&pb, progress, msg);
            })
        })
        .await?;

        match result {
            Ok(bb) => {
                bad_block_count = Some(bb.bad_blocks.len() as u64);
                if total_blocks.is_none() {
                    total_blocks = Some(bb.total_blocks);
                }
                bad_block_positions.extend(bb.bad_blocks.iter());
                if let OutputMode::Human = mode {
                    eprintln!("  Bad blocks: {} / {}", bb.bad_blocks.len(), bb.total_blocks);
                }
            }
            Err(e) => {
                eprintln!("  Bad block test error: {}", e);
            }
        }
    }

    // Thermal test
    if let Some(secs) = test_thermal {
        if !STOP_FLAG.load(Ordering::Relaxed) {
            let mount_c = mount_point.clone();
            let pb = make_progress_bar(mode, "Thermal test");

            let result = tokio::task::spawn_blocking(move || {
                crate::test::thermal::run_thermal_test(&mount_c, secs, |progress, msg, _spd| {
                    eprint_progress(&pb, progress, msg);
                })
            })
            .await?;

            match result {
                Ok(th) => {
                    if let OutputMode::Human = mode {
                        eprintln!(
                            "  Thermal: {:.1} MB/s -> {:.1} MB/s ({:.0}% degradation, risk: {:?})",
                            th.avg_speed_first_minute,
                            th.avg_speed_last_minute,
                            th.speed_degradation,
                            th.thermal_risk
                        );
                    }
                }
                Err(e) => {
                    eprintln!("  Thermal test error: {}", e);
                }
            }
        }
    }

    let test_duration_secs = start_time.elapsed().as_secs();

    // Calculate scores
    let score = crate::report::score::calculate_score(
        claimed_capacity,
        real_capacity,
        seq_read_speed,
        seq_write_speed,
        speed_stability,
        speed_drops,
        bad_block_count,
        total_blocks,
    );

    let report_id = uuid::Uuid::new_v4().to_string();
    let test_date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let details_json = serde_json::to_string(&speed_samples).unwrap_or_else(|_| "[]".to_string());

    // Save to DB
    if !no_save {
        let report_detail = ReportDetail {
            id: report_id.clone(),
            drive_name: drive_name.clone(),
            drive_serial: drive_info.serial.clone(),
            claimed_capacity_bytes: claimed_capacity,
            test_date: test_date.clone(),
            total_score: score.total,
            capacity_score: score.capacity_score,
            speed_score: score.speed_score,
            stability_score: score.stability_score,
            badblock_score: score.badblock_score,
            real_capacity_bytes: real_capacity,
            seq_read_speed,
            seq_write_speed,
            random_read_iops,
            random_write_iops,
            speed_stability,
            bad_block_count,
            total_blocks,
            test_duration_secs: Some(test_duration_secs),
            details_json: Some(details_json.clone()),
        };

        let db = Database::open(db_path)?;
        db.save_report(&report_detail)?;
    }

    // Export HTML if requested
    if let Some(html_path) = export_html {
        let html = crate::report::html::generate_html_report(
            &drive_name,
            &test_date,
            claimed_capacity as f64 / 1_000_000_000.0,
            real_capacity.map(|v| v as f64 / 1_000_000_000.0),
            seq_read_speed,
            seq_write_speed,
            random_read_iops,
            random_write_iops,
            speed_stability,
            bad_block_count.unwrap_or(0),
            total_blocks.unwrap_or(0),
            score.total,
            &details_json,
            "[]",
        );
        std::fs::write(html_path, &html)?;
        if let OutputMode::Human = mode {
            eprintln!("HTML report saved to: {}", html_path);
        }
    }

    let test_result = TestResult {
        report_id,
        drive_name,
        claimed_capacity,
        real_capacity,
        seq_read_speed,
        seq_write_speed,
        random_read_iops,
        random_write_iops,
        speed_stability,
        speed_samples,
        bad_block_count,
        total_blocks,
        bad_block_positions,
        total_score: score.total,
        capacity_score: score.capacity_score,
        speed_score: score.speed_score,
        stability_score: score.stability_score,
        badblock_score: score.badblock_score,
        test_duration_secs,
    };

    match mode {
        OutputMode::Json => {
            print_success(mode, &test_result);
        }
        OutputMode::Human => {
            println!();
            println!("=== Test Results ===");
            println!("Score: {}/100 ({:?})", score.total, score.grade);
            println!(
                "  Capacity: {}/35  Speed: {}/25  Stability: {}/15  BadBlock: {}/25",
                score.capacity_score, score.speed_score, score.stability_score, score.badblock_score
            );
            println!("Duration: {}s", test_duration_secs);
            if !no_save {
                println!("Report ID: {}", test_result.report_id);
            }
        }
    }

    Ok(())
}

fn cmd_report_list(mode: OutputMode, db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let reports = db.list_reports()?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &reports);
        }
        OutputMode::Human => {
            if reports.is_empty() {
                println!("No reports found.");
                return Ok(());
            }
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["ID", "Drive", "Date", "Score"]);
            for r in &reports {
                table.add_row(vec![
                    &r.id[..8],
                    &r.drive_name,
                    &r.test_date,
                    &r.total_score.to_string(),
                ]);
            }
            println!("{table}");
        }
    }
    Ok(())
}

fn cmd_report_show(
    mode: OutputMode,
    db_path: &str,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let report = db.get_report(id)?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &report);
        }
        OutputMode::Human => {
            println!("Report: {}", report.id);
            println!("Drive:  {} ({})", report.drive_name, report.drive_serial);
            println!("Date:   {}", report.test_date);
            println!("Score:  {}/100", report.total_score);
            println!(
                "  Capacity: {}/35  Speed: {}/25  Stability: {}/15  BadBlock: {}/25",
                report.capacity_score, report.speed_score, report.stability_score, report.badblock_score
            );
            if let Some(real) = report.real_capacity_bytes {
                println!(
                    "Capacity: {} real / {} claimed",
                    info::format_capacity(real),
                    info::format_capacity(report.claimed_capacity_bytes)
                );
            }
            if let Some(r) = report.seq_read_speed {
                println!("Seq Read:  {:.1} MB/s", r);
            }
            if let Some(w) = report.seq_write_speed {
                println!("Seq Write: {:.1} MB/s", w);
            }
            if let Some(ri) = report.random_read_iops {
                println!("4K Read:   {:.0} IOPS", ri);
            }
            if let Some(wi) = report.random_write_iops {
                println!("4K Write:  {:.0} IOPS", wi);
            }
            if let Some(s) = report.speed_stability {
                println!("Stability: {:.0}%", s * 100.0);
            }
            if let Some(bb) = report.bad_block_count {
                println!(
                    "Bad blocks: {} / {}",
                    bb,
                    report.total_blocks.unwrap_or(0)
                );
            }
            if let Some(d) = report.test_duration_secs {
                println!("Duration:  {}s", d);
            }
        }
    }
    Ok(())
}

fn cmd_report_export(
    mode: OutputMode,
    db_path: &str,
    id: &str,
    html_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let report = db.get_report(id)?;

    let speed_samples = report.details_json.as_deref().unwrap_or("[]");
    let html = crate::report::html::generate_html_report(
        &report.drive_name,
        &report.test_date,
        report.claimed_capacity_bytes as f64 / 1_000_000_000.0,
        report.real_capacity_bytes.map(|v| v as f64 / 1_000_000_000.0),
        report.seq_read_speed,
        report.seq_write_speed,
        report.random_read_iops,
        report.random_write_iops,
        report.speed_stability,
        report.bad_block_count.unwrap_or(0),
        report.total_blocks.unwrap_or(0),
        report.total_score,
        speed_samples,
        "[]",
    );

    std::fs::write(html_path, &html)?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &serde_json::json!({"path": html_path}));
        }
        OutputMode::Human => {
            println!("HTML report exported to: {}", html_path);
        }
    }
    Ok(())
}

fn cmd_report_delete(
    mode: OutputMode,
    db_path: &str,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    db.delete_report(id)?;

    match mode {
        OutputMode::Json => {
            print_success(mode, &serde_json::json!({"deleted": id}));
        }
        OutputMode::Human => {
            println!("Report {} deleted.", id);
        }
    }
    Ok(())
}

fn make_progress_bar(mode: OutputMode, prefix: &str) -> Option<ProgressBar> {
    match mode {
        OutputMode::Human => {
            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(&format!(
                        "{{spinner:.green}} {} [{{bar:40.cyan/blue}}] {{pos}}% {{msg}}",
                        prefix
                    ))
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_draw_target(indicatif::ProgressDrawTarget::stderr());
            Some(pb)
        }
        OutputMode::Json => None,
    }
}

fn eprint_progress(pb: &Option<ProgressBar>, progress: f64, msg: &str) {
    if let Some(pb) = pb {
        pb.set_position((progress * 100.0) as u64);
        pb.set_message(msg.to_string());
        if progress >= 1.0 {
            pb.finish_and_clear();
        }
    }
}

// ============================================================================
// UBench v1.0 — industry-standard benchmark command
// ============================================================================

async fn cmd_bench(
    mode: OutputMode,
    mount: &str,
    profile: crate::bench::BenchProfile,
    out_path: Option<&str>,
    html_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::bench;

    // Sanity check the mount point exists and is writable
    let mount_path = std::path::PathBuf::from(mount);
    if !mount_path.exists() {
        return Err(format!("挂载点不存在: {}", mount).into());
    }

    if mode == OutputMode::Human {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║  UBench v{} — 行业标准 U盘跑分                       ║", env!("CARGO_PKG_VERSION"));
        println!("║  绕过 OS 缓存的真实速度测量 (Direct I/O)                       ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();
        println!("  目标盘     : {}", mount);
        println!("  测试模式   : {} ({})", profile.as_str().to_uppercase(),
            if profile == bench::BenchProfile::Quick { "~1 分钟,4 项" } else { "~10 分钟,6 项" });
        println!("  测试项数   : {}", profile.tests().len());
        println!();
    }

    // Run the benchmark in a blocking task
    let mount_owned = mount.to_string();
    let pb = if mode == OutputMode::Human {
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{elapsed_precise}] {bar:40.cyan/blue} {pos:>3}% {msg}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        Some(pb)
    } else {
        None
    };
    let pb_clone = pb.clone();

    let outcome = tokio::task::spawn_blocking(move || {
        bench::run_bench(&mount_owned, profile, |p, msg| {
            if let Some(pb) = &pb_clone {
                pb.set_position((p * 100.0) as u64);
                pb.set_message(msg.to_string());
            }
        })
    })
    .await??;

    if let Some(pb) = &pb {
        pb.finish_and_clear();
    }

    // Build the report
    let score = bench::score_report(&outcome);

    // Best-effort drive info
    let drive_info = collect_drive_info(mount);
    let interface_info = collect_interface_info(mount);
    let host_info = collect_host_info();

    let report = bench::BenchReport::build(outcome, score, host_info, drive_info, interface_info);

    // Output: human or JSON
    if mode == OutputMode::Json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_bench_human(&report);
    }

    // Save JSON report if requested
    if let Some(path) = out_path {
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
        if mode == OutputMode::Human {
            println!();
            println!("  报告已保存: {}", path);
            println!("  (含 SHA256 签名,任何人可验证)");
        }
    }

    // Export HTML report if requested
    if let Some(path) = html_path {
        let html = crate::report::html::generate_bench_html(&report);
        std::fs::write(path, &html)?;
        if mode == OutputMode::Human {
            println!();
            println!("  HTML 报告已保存: {}", path);
            println!("  (打开即可分享截图)");
        }
    }

    Ok(())
}

fn collect_host_info() -> crate::bench::report::HostInfo {
    crate::bench::report::HostInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname: hostname_best_effort(),
    }
}

fn hostname_best_effort() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn collect_drive_info(mount: &str) -> crate::bench::report::DriveInfo {
    // Try to find from list_usb_drives, fall back to defaults
    let drives = detect::list_usb_drives().unwrap_or_default();
    let mount_norm = normalize_mount(mount);
    let found = drives.iter().find(|d| normalize_mount(&d.mount_point) == mount_norm);
    if let Some(d) = found {
        crate::bench::report::DriveInfo {
            mount_point: d.mount_point.clone(),
            label: d.name.clone(),
            file_system: d.file_system.clone(),
            claimed_capacity_bytes: d.capacity_bytes,
        }
    } else {
        crate::bench::report::DriveInfo {
            mount_point: mount.to_string(),
            label: String::new(),
            file_system: String::new(),
            claimed_capacity_bytes: 0,
        }
    }
}

fn normalize_mount(s: &str) -> String {
    s.trim_end_matches(['\\', '/'])
        .to_uppercase()
}

fn collect_interface_info(mount: &str) -> crate::bench::report::InterfaceInfo {
    crate::disk::device_info::detect_for_mount(mount)
}

fn print_bench_human(report: &crate::bench::BenchReport) {
    println!();
    println!("┌─ 盘信息 ─────────────────────────────────────────────────────");
    println!("│  挂载点: {}", report.drive.mount_point);
    if !report.drive.label.is_empty() {
        println!("│  标签  : {}", report.drive.label);
    }
    if report.drive.claimed_capacity_bytes > 0 {
        println!("│  容量  : {:.1} GiB", report.drive.claimed_capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
    }
    if !report.drive.file_system.is_empty() {
        println!("│  文件系统: {}", report.drive.file_system);
    }
    if !report.interface.model.is_empty() {
        println!("│  型号  : {}", report.interface.model);
    }
    println!("│  类型  : {}", report.interface.device_class.label());
    println!("│  接口  : {} {}", report.interface.bus_type,
        report.interface.usb_version.as_deref().unwrap_or(""));
    println!("└──────────────────────────────────────────────────────────────");
    println!();
    println!("┌─ 测试结果 ───────────────────────────────────────────────────");
    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL);
    table.set_header(vec!["测试项", "中位数", "最小", "最大", "稳定性"]);
    for m in &report.outcome.measurements {
        let cv_str = if m.cv == 0.0 {
            "-".to_string()
        } else {
            format!("{:.1}%", m.cv * 100.0)
        };
        table.add_row(vec![
            m.label.clone(),
            format!("{:.1} {}", m.median, m.unit),
            format!("{:.1}", m.min),
            format!("{:.1}", m.max),
            cv_str,
        ]);
    }
    println!("{}", table);
    println!();
    println!("┌─ UBench Score ───────────────────────────────────────────────");
    let mut t2 = comfy_table::Table::new();
    t2.load_preset(comfy_table::presets::UTF8_FULL);
    t2.set_header(vec!["项目", "测得", "目标(满分)", "权重", "得分"]);
    for s in &report.score.sub_scores {
        t2.add_row(vec![
            s.label.clone(),
            format!("{:.1} {}", s.measured, s.unit),
            format!("{:.0} {}", s.target, s.unit),
            format!("{:.0}%", s.weight_pct),
            format!("{}/100", s.score),
        ]);
    }
    println!("{}", t2);
    println!();
    println!("  ▶▶  UBench Score (UBS): {} / 100", report.score.ubs);
    println!("  ▶▶  Grade            : {}", report.score.grade.label());
    println!();
    println!("  签名: sha256:{}", &report.signature[..32]);
    println!("  规则版本: {}", report.formula_version);
    println!();
}

