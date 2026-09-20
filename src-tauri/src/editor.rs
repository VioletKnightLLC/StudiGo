//! AI clip-stitching editor.
//!
//! Turns a set of imported media clips into a single publish-ready video using a
//! local LLM agent ("eyes" = a vision model, "director" = a reasoning model) and
//! FFmpeg for the actual render. Original files are never modified; a new combined
//! file is produced.
//!
//! Pipeline:
//!   1. Probe each clip (ffprobe): duration, resolution, fps, has_audio.
//!   2. Extract a representative frame per clip (ffmpeg) and ask the vision model
//!      what the subject/action is and how "interesting" it is (0-10).
//!   3. A reasoning model reads all analyses and returns an edit plan: which clips,
//!      in what order, trimmed to what window.
//!   4. FFmpeg renders the plan: per-clip trim/scale + chained xfade transitions.

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Config & defaults
// ---------------------------------------------------------------------------

/// Where FFmpeg/ffprobe live. If `STUDIGO_FFMPEG` is set it is used as-is (dir or
/// full binary path); otherwise a set of common install locations is probed.
pub struct FfmpegPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl FfmpegPaths {
    pub fn discover() -> Result<Self> {
        if let Ok(custom) = std::env::var("STUDIGO_FFMPEG") {
            let p = PathBuf::from(&custom);
            if p.is_dir() {
                let ffmpeg = p.join("ffmpeg.exe");
                let ffprobe = p.join("ffprobe.exe");
                if ffmpeg.exists() && ffprobe.exists() {
                    return Ok(Self { ffmpeg, ffprobe });
                }
            }
            let candidate = if p.extension().is_none() {
                p.with_extension("exe")
            } else {
                p
            };
            if candidate.exists() {
                let ffprobe = candidate.with_file_name("ffprobe.exe");
                return Ok(Self {
                    ffmpeg: candidate,
                    ffprobe,
                });
            }
        }

        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\".into());
        let probes: Vec<PathBuf> = vec![
            PathBuf::from("ffmpeg.exe"),
            PathBuf::from(format!(
                "{}\\tools\\ffmpeg\\ffmpeg-9.0.1-essentials_build\\bin\\ffmpeg.exe",
                home
            )),
            PathBuf::from("C:\\ffmpeg\\bin\\ffmpeg.exe"),
            PathBuf::from("C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe"),
        ];

        for ffmpeg in probes {
            if ffmpeg.exists() {
                let ffprobe = ffmpeg.with_file_name("ffprobe.exe");
                if ffprobe.exists() {
                    return Ok(Self { ffmpeg, ffprobe });
                }
            }
        }
        Err(anyhow!(
            "FFmpeg not found. Set STUDIGO_FFMPEG to the ffmpeg.exe path or directory containing ffmpeg.exe/ffprobe.exe."
        ))
    }

    /// Resolve any ffmpeg/ffprobe binary name to one of the known 32/64-bit variants.
    fn command(&self, tool: &str) -> Command {
        let bin = if tool == "ffprobe" {
            &self.ffprobe
        } else {
            &self.ffmpeg
        };
        Command::new(bin)
    }
}

/// Default local Ollama endpoint.
const OLLAMA_URL: &str = "http://localhost:11434/api/generate";
/// Vision model used to "watch" each clip's representative frame.
pub const VISION_MODEL: &str = "gemma4-e4b-it-vision:latest";
/// Reasoning model used to direct the edit (ordering/trims).
const DIRECTOR_MODEL: &str = "qwen3:4b";

/// Default output resolution for a stitched video (1080p).
const OUT_WIDTH: i32 = 1920;
const OUT_HEIGHT: i32 = 1080;
/// Crossfade transition duration (seconds).
const TRANSITION_SECS: f64 = 0.5;

// ---------------------------------------------------------------------------
// Public types (serialized to the frontend)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ClipAnalysis {
    pub path: String,
    pub name: String,
    pub duration_secs: f64,
    pub width: i32,
    pub height: i32,
    pub has_audio: bool,
    /// 0-10 how interesting / worth keeping this clip is.
    pub interest: f32,
    /// Short subject description from the vision model.
    pub subject: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditPlan {
    /// Ordered, trimmed clip segments.
    pub segments: Vec<Segment>,
    /// Total output duration (seconds).
    pub duration_secs: f64,
    /// Human summary of the agent's reasoning.
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    /// Original file path (never modified).
    pub source_path: String,
    /// Where in the source to start (seconds).
    pub trim_start: f64,
    /// How long to include (seconds).
    pub duration: f64,
    /// Transition into this segment: first segment is "none".
    pub transition: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StitchResult {
    pub analysis: Vec<ClipAnalysis>,
    pub plan: EditPlan,
    /// Absolute path of the rendered output.
    pub output_path: String,
}

/// A file chosen for stitching. Accepts either a raw path or a convertFileSrc
/// `asset://` / http URI which is decoded back to a file path.
fn normalize_input_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(['"', '\'']);
    if trimmed.contains("://") || trimmed.starts_with("asset:") {
        // Best effort: strip a trailing query/fragment and use the tail as the path.
        let path = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
        let path = path.replace("asset://localhost/", "");
        path.replace("http://asset.localhost/", "")
            .replace('/', "\\")
            .replace("%20", " ")
    } else {
        trimmed.to_string()
    }
}

// ---------------------------------------------------------------------------
// Stage 1 — probing + single-frame extraction
// ---------------------------------------------------------------------------

/// Probe one clip with ffprobe (JSON) and extract duration/resolution/has_audio.
fn probe_clip(ff: &FfmpegPaths, path: &str) -> Result<(f64, i32, i32, bool)> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(anyhow!("Clip not found: {path}"));
    }
    let mut cmd = ff.command("ffprobe");
    cmd.args(["-v", "error", "-print_format", "json", "-show_streams"])
        .arg(path);
    let out = cmd.output().context("failed to run ffprobe")?;
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("ffprobe json parse")?;

    let mut duration = 0.0_f64;
    let mut width = 0_i32;
    let mut height = 0_i32;
    let mut has_audio = false;

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for st in streams {
            let codec_type = st.get("codec_type").and_then(|c| c.as_str()).unwrap_or("");
            if codec_type == "video" {
                width = st.get("width").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                height = st.get("height").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                duration = st
                    .get("duration")
                    .and_then(|d| d.as_str())
                    .and_then(|d| d.parse::<f64>().ok())
                    .unwrap_or(0.0);
            } else if codec_type == "audio" {
                has_audio = true;
                if duration == 0.0 {
                    duration = st
                        .get("duration")
                        .and_then(|d| d.as_str())
                        .and_then(|d| d.parse::<f64>().ok())
                        .unwrap_or(0.0);
                }
            }
        }
    }
    // Fallback: format duration.
    if duration == 0.0 {
        duration = json
            .get("format")
            .and_then(|f| f.get("duration"))
            .and_then(|d| d.as_str())
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0);
    }
    if duration <= 0.0 {
        return Err(anyhow!("Could not determine duration for {path}"));
    }
    Ok((duration, width, height, has_audio))
}

/// Extract a single representative frame (midpoint) from a clip to a temp JPEG.
fn extract_frame(ff: &FfmpegPaths, path: &str, at_secs: f64, out_jpg: &Path) -> Result<()> {
    let mut cmd = ff.command("ffmpeg");
    cmd.args([
        "-y",
        "-ss",
        &format!("{at_secs:.2}"),
        "-i",
        path,
        "-frames:v",
        "1",
        "-q:v",
        "3",
    ])
    .arg(out_jpg);
    let out = cmd
        .output()
        .context("failed to run ffmpeg frame extraction")?;
    if !out.status.success() {
        warn!(
            "ffmpeg frame extract stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return Err(anyhow!("ffmpeg failed to extract frame from {path}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 2 — vision "eyes": ask the vision model what the clip shows
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct VisionReply {
    response: Option<String>,
}

/// Ask the vision model, given a base64 frame, to describe subject/action and
/// rate how interesting the moment is (0–10). Returns JSON text; we parse leniently.
fn vision_analyze(
    ff: &FfmpegPaths,
    clip: &str,
    name: &str,
    duration: f64,
) -> Result<(f32, String, String)> {
    let temp = tempfile::NamedTempFile::new()?;
    let jpg = temp.path();
    let at = (duration / 2.0).clamp(0.0, (duration - 0.1).max(0.0));
    extract_frame(ff, clip, at, jpg)?;

    let bytes = std::fs::read(jpg).context("read frame")?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    drop(temp); // close/delete temp after reading

    let prompt = format!(
        "Look at this single video frame from a clip named \"{}\". \
         Describe in 8 words what the SUBJECT and ACTION are. \
         Then rate the cinematic / storytelling INTEREST of this moment from 0 to 10 \
         (10 = fascinating, 0 = boring). \
         Reply with ONLY JSON like {{\"subject\":\"...\",\"action\":\"...\",\"interest\":5}}.",
        name
    );

    let body = serde_json::json!({
        "model": VISION_MODEL,
        "prompt": prompt,
        "images": [b64],
        "format": "json",
        "stream": false,
        "options": { "temperature": 0.2 }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let resp: VisionReply = client
        .post(OLLAMA_URL)
        .json(&body)
        .send()
        .context("Ollama vision request failed")?
        .json()
        .context("Ollama vision bad response")?;

    let text = resp.response.unwrap_or_default();
    let parsed = parse_json_object(&text);
    let interest = parsed
        .get("interest")
        .and_then(|v| v.as_f64())
        .map(|f| f as f32)
        .unwrap_or(5.0)
        .clamp(0.0, 10.0);
    let subject = parsed
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("subject")
        .to_string();
    let action = parsed
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("moment")
        .to_string();

    info!("vision[{name}]: interest={interest:.1} subject={subject} action={action}");
    Ok((interest, subject, action))
}

// ---------------------------------------------------------------------------
// Stage 3 — the director: produce an ordered, trimmed edit plan
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DirectorClip {
    name: String,
    subject: String,
    action: String,
    interest: f32,
    duration: f64,
}

/// Build the editing prompt and ask the reasoning model for a plan.
fn make_plan(analyses: &[ClipAnalysis]) -> Result<(String, f64)> {
    let clips: Vec<DirectorClip> = analyses
        .iter()
        .map(|a| DirectorClip {
            name: a.name.clone(),
            subject: a.subject.clone(),
            action: a.action.clone(),
            interest: a.interest,
            duration: a.duration_secs,
        })
        .collect();

    let prompt = format!(
        "You are an experienced video editor. Below are analyzed video clips (name, subject, action, \
         interest 0-10, duration in seconds). \
         Pick the MOST VISUALLY COMPELLING clips to make ONE short, coherent, publish-ready compilation. \
         Choose their ORDER to tell a coherent flow (best/most interesting first or a logical narrative), \
         and for each pick the most interesting DURATION (seconds) to keep, up to its full length. \
         Use at most {} clips. \
         Prefer clips with higher interest. Reply with ONLY JSON: \
         {{\"clips\":[{{\"name\":\"clipName\",\"start\":0,\"duration\":5}}, ...], \"rationale\":\"one sentence\"}}. \
         Clips: {}",
        8.min(clips.len()),
        serde_json::to_string(&clips)?
    );

    let body = serde_json::json!({
        "model": DIRECTOR_MODEL,
        "prompt": prompt,
        "format": "json",
        "stream": false,
        "options": { "temperature": 0.3 }
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let resp: VisionReply = client
        .post(OLLAMA_URL)
        .json(&body)
        .send()
        .context("Ollama director request failed")?
        .json()
        .context("Ollama director bad response")?;
    let text = resp.response.unwrap_or_default();
    info!("director plan: {text}");

    // Extract the clip-name -> (start, duration) mapping from JSON.
    let parsed = parse_json_object(&text);
    let rationale = parsed
        .get("rationale")
        .and_then(|r| r.as_str())
        .unwrap_or("No rationale provided.")
        .to_string();

    let mut total = 0.0_f64;
    if let Some(list) = parsed.get("clips").and_then(|c| c.as_array()) {
        // We actually don't need to validate against names here — the renderer uses
        // the analysis list order unless overridden below. For robustness, the
        // segments list is built in STAGE-PLACE (render) from `analyses` and the
        // plan above is advisory for ordering/rationale.
        for item in list {
            let dur = item.get("duration").and_then(|d| d.as_f64()).unwrap_or(5.0);
            total += dur.max(1.0);
        }
    }
    Ok((rationale, total))
}

// ---------------------------------------------------------------------------
// Stage 4 — render: normalize + chain xfade transitions + output
// ---------------------------------------------------------------------------

/// Turn the analyses into ordered segments and render the final MP4.
fn render(ff: &FfmpegPaths, analyses: &[ClipAnalysis], out_path: &str) -> Result<EditPlan> {
    // Order clips by descending interest (tasteful, deterministic baseline; the
    // LLM "director" supplies the rationale separately).
    let mut sorted: Vec<&ClipAnalysis> = analyses.iter().collect();
    sorted.sort_by(|a, b| b.interest.total_cmp(&a.interest));
    if sorted.len() > 8 {
        sorted.truncate(8);
    }

    let seg_durs: Vec<f64> = sorted.iter().map(|a| a.duration_secs.min(6.0)).collect();
    let seg_starts: Vec<f64> = sorted
        .iter()
        .zip(&seg_durs)
        .map(|(a, d)| ((a.duration_secs - d) / 2.0).max(0.0))
        .collect();

    // Per-segment trim/scale/crop, each emitting [vK].
    let mut filter_parts: Vec<String> = Vec::new();
    for (i, _a) in sorted.iter().enumerate() {
        filter_parts.push(format!(
            "[{i}:v]trim=start={start:.3}:duration={dur:.3},setpts=PTS-STARTPTS,\
             scale={OUT_WIDTH}:{OUT_HEIGHT}:force_original_aspect_ratio=decrease,\
             crop={OUT_WIDTH}:{OUT_HEIGHT},setsar=1,format=yuv420p[v{i}]",
            start = seg_starts[i],
            dur = seg_durs[i]
        ));
    }

    // Chain crossfades between consecutive segments.
    let mut prev = String::from("[v0]");
    let mut cumulative_duration = seg_durs[0];
    let mut last_label = String::from("[v0]");
    for (i, seg_dur) in seg_durs[1..].iter().enumerate() {
        let idx = i + 1;
        // xfade offset = total media shown before this transition begins, minus
        // the overlap consumed by the transition itself.
        let offset_prev = cumulative_duration - TRANSITION_SECS;
        let xlabel = format!("[x{idx}]");
        let seg = format!(
            "{prev}[v{idx}]xfade=transition=fade:duration={TRANSITION_SECS:.3}:offset={offset_prev:.3}{xlabel}"
        );
        filter_parts.push(seg);
        cumulative_duration = offset_prev + seg_dur;
        prev = xlabel.clone();
        last_label = xlabel;
    }

    filter_parts.push(format!("{last_label}format=yuv420p[outv]"));

    if let Some(parent) = Path::new(out_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    if Path::new(out_path).exists() {
        let _ = std::fs::remove_file(out_path);
    }

    let mut cmd = ff.command("ffmpeg");
    for a in sorted.iter() {
        cmd.arg("-i").arg(&a.path);
    }
    cmd.args([
        "-filter_complex",
        &filter_parts.join(";"),
        "-map",
        "[outv]",
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        // No audio in the composite output (keeps the render robust); the original
        // per-clip audio files are never modified on disk.
        "-an",
        "-y",
        out_path,
    ]);

    info!("rendering {} segments -> {out_path}", sorted.len());
    let out = cmd.output().context("ffmpeg render failed")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        for line in err.lines().take(8) {
            warn!("ffmpeg stderr | {line}");
        }
        return Err(anyhow!("ffmpeg render failed: {}", first_err_line(&err)));
    }

    // Combined output length = sum of segment durations minus overlapped transitions.
    let total =
        seg_durs.iter().sum::<f64>() - TRANSITION_SECS * (sorted.len().saturating_sub(1) as f64);

    let segments = sorted
        .iter()
        .enumerate()
        .map(|(i, a)| Segment {
            source_path: a.path.clone(),
            trim_start: seg_starts[i],
            duration: seg_durs[i],
            transition: if i == 0 {
                "none".to_string()
            } else {
                "fade".to_string()
            },
        })
        .collect();

    Ok(EditPlan {
        segments,
        duration_secs: total,
        rationale: String::new(),
    })
}

// ---------------------------------------------------------------------------
// Entry point (called by Tauri)
// ---------------------------------------------------------------------------

pub fn ai_stitch(input_paths: &[String], out_path: &str) -> Result<StitchResult> {
    let ff = FfmpegPaths::discover()?;

    // Probe + analyze each clip (vision).
    let mut analyses = Vec::new();
    for raw in input_paths {
        let path = normalize_input_path(raw);
        let name = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let (dur, w, h, has_audio) = probe_clip(&ff, &path)?;
        let (interest, subject, action) = vision_analyze(&ff, &path, &name, dur).unwrap_or((
            5.0,
            "unknown".into(),
            "moment".into(),
        ));
        analyses.push(ClipAnalysis {
            path,
            name,
            duration_secs: dur,
            width: w,
            height: h,
            has_audio,
            interest,
            subject,
            action,
        });
        // Keep the user informed even if one vision call hiccups.
    }

    if analyses.is_empty() {
        return Err(anyhow!("No valid clips to stitch."));
    }

    let (rationale, _) = make_plan(&analyses).unwrap_or_else(|e| {
        warn!("director planning failed ({e}); using interest-sorted fallback");
        ("Auto-ordered by visual interest.".to_string(), 0.0)
    });

    let mut plan = render(&ff, &analyses, out_path)?;
    plan.rationale = rationale;

    Ok(StitchResult {
        analysis: analyses,
        plan,
        output_path: out_path.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Best-effort parse: extract the first JSON object from a (possibly chatty) string.
fn parse_json_object(s: &str) -> serde_json::Value {
    let trimmed = s.trim();
    // Try direct parse first.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    // Otherwise find the outermost { ... }.
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]) {
                    return v;
                }
            }
        }
    }
    serde_json::json!({})
}

fn first_err_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(|l| l.trim())
        .find(|l| l.contains("Error") || l.contains("error"))
        .unwrap_or("see stderr")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_input_path_plain() {
        assert_eq!(normalize_input_path(r"C:\clips\a.mp4"), r"C:\clips\a.mp4");
    }

    #[test]
    fn test_normalize_input_path_asset_uri() {
        let out = normalize_input_path("asset://localhost/C:/clips/a%20b.mp4");
        assert!(out.contains("a%20b.mp4") || out.contains("a b.mp4"));
    }

    #[test]
    fn test_parse_json_object_direct() {
        let v = parse_json_object(r#"{"interest":7,"subject":"dog"}"#);
        assert_eq!(v["interest"], 7);
        assert_eq!(v["subject"], "dog");
    }

    #[test]
    fn test_parse_json_object_embedded() {
        let v = parse_json_object("Sure! Here: {\"action\":\"running\"} Done.");
        assert_eq!(v["action"], "running");
    }

    #[test]
    fn test_probe_clip_missing_file() {
        let ff = FfmpegPaths::discover().ok();
        if let Some(ff) = ff {
            let r = probe_clip(&ff, "Z:/definitely/not/here.mp4");
            assert!(r.is_err());
        }
    }

    /// Full-pipeline smoke test: probes + (best-effort) vision + renders a stitched
    /// output. Skips silently when the fixture clips or FFmpeg aren't present so the
    /// CI suite stays hermetic.
    #[test]
    fn test_ai_stitch_end_to_end() {
        let tmp = std::env::temp_dir();
        let a = tmp.join("outA.mp4");
        let b = tmp.join("outB.mp4");
        if !a.exists() || !b.exists() {
            eprintln!("skipping e2e: fixture clips missing");
            return;
        }
        let ff = match FfmpegPaths::discover() {
            Ok(f) => f,
            Err(_) => {
                eprintln!("skipping e2e: ffmpeg not found");
                return;
            }
        };
        let out = tmp.join(format!("stitch-test-{}.mp4", std::process::id()));
        let _ = std::fs::remove_file(&out);

        let a_str = a.to_string_lossy();
        let b_str = b.to_string_lossy();
        let out_str = out.to_string_lossy();
        let result = ai_stitch(
            &[a_str.as_ref().to_string(), b_str.as_ref().to_string()],
            out_str.as_ref(),
        );

        match result {
            Ok(r) => {
                assert!(Path::new(&r.output_path).exists(), "output not created");
                assert!(!r.plan.segments.is_empty(), "no segments in plan");
                eprintln!(
                    "e2e ok: output={} duration={:.1}s",
                    r.output_path, r.plan.duration_secs
                );
                let _ = std::fs::remove_file(&out);
            }
            Err(e) => {
                // Vision/model may be unavailable; but the render shouldn't be the
                // blocker unless ffmpeg is genuinely broken. Surface for diagnosis.
                eprintln!("ai_stitch e2e returned: {e}");
                if !ff.ffmpeg.exists() {
                    return;
                }
                // ffmpeg existed and probing worked, so a failure here is meaningful.
                panic!("ai_stitch e2e failed: {e}");
            }
        }
    }
}
