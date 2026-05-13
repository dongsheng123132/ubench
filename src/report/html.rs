/// Generate a UBench v1.0 bench HTML report with ECharts visualization.
/// Called by `ubench bench E:\ --export-html report.html`.
pub fn generate_bench_html(report: &crate::bench::report::BenchReport) -> String {
    use crate::bench::report::DeviceClass;

    let score = report.score.ubs;
    let grade = report.score.grade.label();
    let date = report.generated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let (score_class, grade_class) = match score {
        85..=100 => ("excellent", "grade-excellent"),
        70..=84  => ("good", "grade-good"),
        50..=69  => ("fair", "grade-fair"),
        25..=49  => ("poor", "grade-poor"),
        _        => ("fail", "grade-fail"),
    };

    // Build bar chart data from sub_scores with weight > 0
    let bar_labels: Vec<String> = report.score.sub_scores.iter()
        .filter(|s| s.weight_pct > 0.0)
        .map(|s| format!("\"{}\"", s.label.replace('"', "'")))
        .collect();
    let bar_scores: Vec<String> = report.score.sub_scores.iter()
        .filter(|s| s.weight_pct > 0.0)
        .map(|s| s.score.to_string())
        .collect();
    let bar_measured: Vec<String> = report.score.sub_scores.iter()
        .filter(|s| s.weight_pct > 0.0)
        .map(|s| format!("{:.1} {}", s.measured, s.unit))
        .collect();

    // Table rows HTML
    let mut rows = String::new();
    for s in report.outcome.measurements.iter() {
        let cv_str = if s.cv == 0.0 {
            "-".to_string()
        } else {
            format!("{:.1}%", s.cv * 100.0)
        };
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{:.1} {}</td><td>{:.1}</td><td>{:.1}</td><td>{}</td></tr>\n",
            s.label, s.median, s.unit, s.min, s.max, cv_str
        ));
    }

    // Sub-score rows
    let mut score_rows = String::new();
    for s in report.score.sub_scores.iter().filter(|s| s.weight_pct > 0.0) {
        let bar_w = s.score;
        let bar_color = if bar_w >= 85 { "#22c55e" } else if bar_w >= 70 { "#3b82f6" } else if bar_w >= 50 { "#eab308" } else { "#ef4444" };
        score_rows.push_str(&format!(
            "<tr><td>{}</td><td>{:.1} {}</td><td>{:.0} {}</td><td>{:.0}%</td><td><div class='sbar-wrap'><div class='sbar' style='width:{}%;background:{}'></div><span>{}/100</span></div></td></tr>\n",
            s.label, s.measured, s.unit, s.target, s.unit, s.weight_pct, bar_w, bar_color, s.score
        ));
    }

    let device_class_label = report.interface.device_class.label();
    let is_usb = !matches!(report.interface.device_class, DeviceClass::NvmeSsd | DeviceClass::SataSsd | DeviceClass::HardDisk);
    let usb_note = if is_usb { "Direct I/O (绕过 OS 缓存)" } else { "Direct I/O" };

    let capacity_str = if report.drive.claimed_capacity_bytes > 0 {
        format!("{:.1} GiB", report.drive.claimed_capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        "N/A".to_string()
    };

    format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>UBench 跑分报告 — {mount}</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
  *{{box-sizing:border-box;margin:0;padding:0}}
  body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#0f172a;color:#e2e8f0;min-height:100vh}}
  .hero{{background:linear-gradient(135deg,#1e3a5f 0%,#0f1f3c 100%);padding:32px 24px 24px;border-bottom:1px solid #1e40af22}}
  .hero h1{{font-size:22px;font-weight:700;color:#60a5fa;letter-spacing:0.5px}}
  .hero .sub{{font-size:13px;color:#94a3b8;margin-top:4px}}
  .container{{max-width:960px;margin:0 auto;padding:24px 16px}}
  .card{{background:#1e293b;border-radius:12px;padding:24px;margin-bottom:20px;border:1px solid #334155}}
  .card h2{{font-size:15px;font-weight:600;color:#94a3b8;text-transform:uppercase;letter-spacing:1px;margin-bottom:16px;border-bottom:1px solid #334155;padding-bottom:10px}}
  .score-wrap{{display:flex;align-items:center;gap:32px;flex-wrap:wrap}}
  .score-num{{font-size:96px;font-weight:900;line-height:1}}
  .score-num.excellent{{color:#22c55e}}
  .score-num.good{{color:#3b82f6}}
  .score-num.fair{{color:#eab308}}
  .score-num.poor{{color:#f97316}}
  .score-num.fail{{color:#ef4444}}
  .score-detail{{flex:1;min-width:200px}}
  .grade-badge{{display:inline-block;padding:6px 16px;border-radius:20px;font-size:16px;font-weight:700;margin-bottom:12px}}
  .grade-excellent{{background:#14532d;color:#22c55e}}
  .grade-good{{background:#1e3a8a;color:#60a5fa}}
  .grade-fair{{background:#713f12;color:#fbbf24}}
  .grade-poor{{background:#7c2d12;color:#fb923c}}
  .grade-fail{{background:#450a0a;color:#ef4444}}
  .meta-grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:12px}}
  .meta-item{{background:#0f172a;border-radius:8px;padding:12px 14px}}
  .meta-label{{font-size:11px;color:#64748b;text-transform:uppercase;letter-spacing:0.5px}}
  .meta-value{{font-size:15px;font-weight:600;color:#e2e8f0;margin-top:2px}}
  table{{width:100%;border-collapse:collapse;font-size:13px}}
  th{{text-align:left;color:#64748b;font-weight:500;padding:8px 10px;border-bottom:1px solid #334155}}
  td{{padding:8px 10px;border-bottom:1px solid #1e293b}}
  tr:last-child td{{border-bottom:none}}
  tr:hover td{{background:#1e3a5f22}}
  .sbar-wrap{{display:flex;align-items:center;gap:8px}}
  .sbar{{height:8px;border-radius:4px;min-width:2px}}
  .sbar-wrap span{{font-size:12px;color:#94a3b8;white-space:nowrap}}
  #barChart{{width:100%;height:280px}}
  .sig{{font-size:11px;color:#475569;margin-top:4px;font-family:monospace}}
  .footer{{text-align:center;color:#334155;font-size:12px;padding:24px}}
  .footer a{{color:#3b82f6;text-decoration:none}}
  .pill{{display:inline-block;background:#1e3a8a;color:#93c5fd;font-size:11px;padding:2px 8px;border-radius:10px;margin-left:6px}}
</style>
</head>
<body>
<div class="hero">
  <div style="max-width:960px;margin:0 auto">
    <h1>UBench — 行业标准 U盘跑分<span class="pill">Direct I/O</span></h1>
    <div class="sub">{mount} &nbsp;·&nbsp; {date} &nbsp;·&nbsp; v{version}</div>
  </div>
</div>

<div class="container">

<div class="card">
  <h2>UBench Score (UBS)</h2>
  <div class="score-wrap">
    <div class="score-num {score_class}">{score}</div>
    <div class="score-detail">
      <div class="grade-badge {grade_class}">{grade}</div>
      <div style="font-size:13px;color:#94a3b8;margin-top:8px">规则版本: {formula}</div>
      <div class="sig">sha256:{sig}</div>
    </div>
  </div>
</div>

<div class="card">
  <h2>设备信息</h2>
  <div class="meta-grid">
    <div class="meta-item"><div class="meta-label">挂载点</div><div class="meta-value">{mount}</div></div>
    <div class="meta-item"><div class="meta-label">类型</div><div class="meta-value">{device_class}</div></div>
    <div class="meta-item"><div class="meta-label">接口</div><div class="meta-value">{bus_type}</div></div>
    <div class="meta-item"><div class="meta-label">容量</div><div class="meta-value">{capacity}</div></div>
    <div class="meta-item"><div class="meta-label">文件系统</div><div class="meta-value">{fs}</div></div>
    <div class="meta-item"><div class="meta-label">测量方式</div><div class="meta-value">{usb_note}</div></div>
    {model_html}
  </div>
</div>

<div class="card">
  <h2>各项分数</h2>
  <div id="barChart"></div>
</div>

<div class="card">
  <h2>详细分数</h2>
  <table>
    <thead><tr><th>测试项</th><th>测得值</th><th>目标(满分)</th><th>权重</th><th>得分</th></tr></thead>
    <tbody>{score_rows}</tbody>
  </table>
</div>

<div class="card">
  <h2>原始测量数据</h2>
  <table>
    <thead><tr><th>测试项</th><th>中位数</th><th>最小</th><th>最大</th><th>稳定性</th></tr></thead>
    <tbody>{rows}</tbody>
  </table>
</div>

</div>

<div class="footer">
  Generated by <a href="https://github.com/dongsheng123132/ubench">UBench</a> — 开源 U盘跑分工具 | MIT License
</div>

<script>
var chart = echarts.init(document.getElementById('barChart'));
chart.setOption({{
  tooltip: {{ trigger: 'axis', formatter: function(p) {{
    var s = p[0]; return s.name + '<br/>' + s.value + '/100';
  }} }},
  grid: {{ left: '180px', right: '40px', top: '20px', bottom: '20px' }},
  xAxis: {{ type: 'value', max: 100, splitLine: {{ lineStyle: {{ color: '#334155' }} }}, axisLabel: {{ color: '#64748b' }} }},
  yAxis: {{ type: 'category', data: [{bar_labels}], axisLabel: {{ color: '#e2e8f0', fontSize: 12 }} }},
  series: [{{
    type: 'bar',
    data: [{bar_scores}],
    barMaxWidth: 24,
    itemStyle: {{ borderRadius: 4, color: function(p) {{
      var v = p.value;
      if (v >= 85) return '#22c55e';
      if (v >= 70) return '#3b82f6';
      if (v >= 50) return '#eab308';
      if (v >= 25) return '#f97316';
      return '#ef4444';
    }} }},
    label: {{ show: true, position: 'right', color: '#94a3b8', formatter: function(p) {{
      var tips = [{bar_measured_js}];
      return p.value + '/100  ' + (tips[p.dataIndex] || '');
    }} }}
  }}]
}});
window.addEventListener('resize', function() {{ chart.resize(); }});
</script>
</body>
</html>"#,
        mount = report.drive.mount_point,
        date = date,
        version = report.ubench_version,
        score = score,
        score_class = score_class,
        grade = grade,
        grade_class = grade_class,
        formula = report.formula_version,
        sig = &report.signature[..32],
        device_class = device_class_label,
        bus_type = report.interface.bus_type,
        capacity = capacity_str,
        fs = if report.drive.file_system.is_empty() { "N/A" } else { &report.drive.file_system },
        usb_note = usb_note,
        model_html = if report.interface.model.is_empty() { String::new() } else {
            format!("<div class='meta-item'><div class='meta-label'>型号</div><div class='meta-value'>{}</div></div>", report.interface.model)
        },
        score_rows = score_rows,
        rows = rows,
        bar_labels = bar_labels.join(","),
        bar_scores = bar_scores.join(","),
        bar_measured_js = bar_measured.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(","),
    )
}

pub fn generate_html_report(
    drive_name: &str,
    test_date: &str,
    claimed_capacity_gb: f64,
    real_capacity_gb: Option<f64>,
    seq_read: Option<f64>,
    seq_write: Option<f64>,
    random_read_iops: Option<f64>,
    random_write_iops: Option<f64>,
    stability: Option<f64>,
    bad_blocks: u64,
    _total_blocks: u64,
    total_score: u32,
    speed_samples_json: &str,
    bad_block_positions_json: &str,
) -> String {
    let score_class = if total_score >= 90 {
        "excellent"
    } else if total_score >= 70 {
        "good"
    } else if total_score >= 50 {
        "fair"
    } else {
        "poor"
    };

    let grade_text = if total_score >= 90 {
        "优秀"
    } else if total_score >= 70 {
        "良好"
    } else if total_score >= 50 {
        "一般"
    } else {
        "差"
    };

    let real_gb = match real_capacity_gb {
        Some(v) => format!("{:.1} GB", v),
        None => "未测试".to_string(),
    };

    let seq_read_str = match seq_read {
        Some(v) => format!("{:.1} MB/s", v),
        None => "N/A".to_string(),
    };

    let seq_write_str = match seq_write {
        Some(v) => format!("{:.1} MB/s", v),
        None => "N/A".to_string(),
    };

    let rand_read_str = match random_read_iops {
        Some(v) => format!("{:.0}", v),
        None => "N/A".to_string(),
    };

    let rand_write_str = match random_write_iops {
        Some(v) => format!("{:.0}", v),
        None => "N/A".to_string(),
    };

    let stability_str = match stability {
        Some(v) => format!("{:.0}%", v * 100.0),
        None => "N/A".to_string(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>UBench 跑分报告 - {drive_name}</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; background: #f5f5f5; }}
  .header {{ background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px; border-radius: 12px; margin-bottom: 20px; }}
  .header h1 {{ margin: 0 0 8px 0; font-size: 24px; }}
  .header p {{ margin: 0; opacity: 0.9; }}
  .card {{ background: white; border-radius: 12px; padding: 24px; margin-bottom: 16px; box-shadow: 0 2px 8px rgba(0,0,0,0.08); }}
  .card h2 {{ margin-top: 0; color: #333; font-size: 18px; border-bottom: 2px solid #eee; padding-bottom: 8px; }}
  .score {{ font-size: 64px; font-weight: bold; text-align: center; }}
  .score.excellent {{ color: #22c55e; }}
  .score.good {{ color: #3b82f6; }}
  .score.fair {{ color: #eab308; }}
  .score.poor {{ color: #ef4444; }}
  .stat-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; }}
  .stat {{ text-align: center; padding: 16px; background: #f8fafc; border-radius: 8px; }}
  .stat-value {{ font-size: 28px; font-weight: bold; color: #1e40af; }}
  .stat-label {{ font-size: 14px; color: #64748b; margin-top: 4px; }}
  .chart {{ width: 100%; height: 300px; }}
  .grade-badge {{ display: inline-block; padding: 4px 12px; border-radius: 20px; font-size: 14px; font-weight: 600; }}
  .grade-excellent {{ background: #dcfce7; color: #166534; }}
  .grade-good {{ background: #dbeafe; color: #1e40af; }}
  .grade-fair {{ background: #fef3c7; color: #92400e; }}
  .grade-poor {{ background: #fee2e2; color: #991b1b; }}
</style>
</head>
<body>
<div class="header">
  <h1>U盘质量检测报告</h1>
  <p>{drive_name} | 检测日期: {test_date}</p>
</div>

<div class="card">
  <h2>综合评分</h2>
  <div class="score {score_class}">{total_score}<span style="font-size:24px">/100</span></div>
  <div style="text-align:center"><span class="grade-badge grade-{score_class}">{grade_text}</span></div>
</div>

<div class="card">
  <h2>检测数据</h2>
  <div class="stat-grid">
    <div class="stat">
      <div class="stat-value">{claimed_gb:.1} GB</div>
      <div class="stat-label">标称容量</div>
    </div>
    <div class="stat">
      <div class="stat-value">{real_gb}</div>
      <div class="stat-label">真实容量</div>
    </div>
    <div class="stat">
      <div class="stat-value">{seq_read_str}</div>
      <div class="stat-label">顺序读取</div>
    </div>
    <div class="stat">
      <div class="stat-value">{seq_write_str}</div>
      <div class="stat-label">顺序写入</div>
    </div>
    <div class="stat">
      <div class="stat-value">{rand_read_str}</div>
      <div class="stat-label">随机读取 IOPS</div>
    </div>
    <div class="stat">
      <div class="stat-value">{rand_write_str}</div>
      <div class="stat-label">随机写入 IOPS</div>
    </div>
    <div class="stat">
      <div class="stat-value">{stability_str}</div>
      <div class="stat-label">速度稳定性</div>
    </div>
    <div class="stat">
      <div class="stat-value">{bad_blocks}</div>
      <div class="stat-label">坏块数量</div>
    </div>
  </div>
</div>

<div class="card">
  <h2>读写速度曲线</h2>
  <div id="speedChart" class="chart"></div>
</div>

<div class="card">
  <h2>坏块分布</h2>
  <div id="badBlockChart" class="chart"></div>
</div>

<script>
var speedData = {speed_samples_json};
if (speedData && speedData.length > 0) {{
  var chart = echarts.init(document.getElementById('speedChart'));
  chart.setOption({{
    tooltip: {{ trigger: 'axis' }},
    legend: {{ data: ['写入速度', '读取速度'] }},
    xAxis: {{ type: 'category', data: speedData.map(s => s.offset_mb + ' MB'), name: '偏移量' }},
    yAxis: {{ type: 'value', name: 'MB/s' }},
    series: [
      {{ name: '写入速度', type: 'line', data: speedData.map(s => s.write_speed), smooth: true, itemStyle: {{ color: '#ef4444' }} }},
      {{ name: '读取速度', type: 'line', data: speedData.map(s => s.read_speed), smooth: true, itemStyle: {{ color: '#3b82f6' }} }}
    ]
  }});
}}

var badData = {bad_block_positions_json};
if (badData && badData.length > 0) {{
  var chart2 = echarts.init(document.getElementById('badBlockChart'));
  var heatData = badData.map((pos, i) => [pos % 100, Math.floor(pos / 100), 1]);
  chart2.setOption({{
    tooltip: {{}},
    xAxis: {{ type: 'category', splitArea: {{ show: true }} }},
    yAxis: {{ type: 'category', splitArea: {{ show: true }} }},
    visualMap: {{ min: 0, max: 1, calculable: false, orient: 'horizontal', left: 'center', bottom: 0, inRange: {{ color: ['#22c55e', '#ef4444'] }} }},
    series: [{{ type: 'heatmap', data: heatData, label: {{ show: false }} }}]
  }});
}} else {{
  document.getElementById('badBlockChart').innerHTML = '<p style="text-align:center;color:#22c55e;font-size:18px;padding:40px">未发现坏块</p>';
}}
</script>
<div style="text-align:center;color:#94a3b8;font-size:12px;margin-top:20px">
  Generated by UBench (https://github.com/dongsheng123132/ubench) | {test_date}
</div>
</body>
</html>"#,
        drive_name = drive_name,
        test_date = test_date,
        total_score = total_score,
        score_class = score_class,
        grade_text = grade_text,
        claimed_gb = claimed_capacity_gb,
        real_gb = real_gb,
        seq_read_str = seq_read_str,
        seq_write_str = seq_write_str,
        rand_read_str = rand_read_str,
        rand_write_str = rand_write_str,
        stability_str = stability_str,
        bad_blocks = bad_blocks,
        speed_samples_json = speed_samples_json,
        bad_block_positions_json = bad_block_positions_json,
    )
}
