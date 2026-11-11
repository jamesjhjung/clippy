//! clippy milestone 1: record + detect kills.
//! Run:  cargo run --release            (record + detect)
//!       cargo run --release -- --calibrate   (dump kill feed crops as .ppm to tune colors)
//! Requires ffmpeg (with ddagrab + h264_amf) on PATH. Windows only.

use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

// ---------- settings (tweak these) ----------
const CAPTURE_FPS: u32 = 60;
const SEGMENT_SECS: u32 = 60;
const OUTPUT_DIR: &str = "recordings";

// Kill feed region as FRACTIONS of the frame (resolution independent). CALIBRATE THESE.
const CROP_X: f32 = 0.72;
const CROP_Y: f32 = 0.05;
const CROP_W: f32 = 0.28;
const CROP_H: f32 = 0.30;

// Detector frame size (downscaled crop) and rate.
const DET_W: usize = 240;
const DET_H: usize = 144;
const DET_FPS: u32 = 10;

// "Highlighted kill feed row" color test. PLACEHOLDER VALUES: tune with --calibrate.
const R_MIN: u8 = 200;
const G_MAX: u8 = 90;
const B_MAX: u8 = 110;
const MIN_MATCH_PIXELS: usize = 150; // pixels that must match to count as a highlight
const COOLDOWN_SECS: f64 = 1.5; // ignore re-triggers within this window
// --------------------------------------------

#[derive(Serialize)]
struct Event {
    kind: &'static str,
    /// Seconds since the recording session started
    t: f64,
    /// Which segment file contains it, and the offset inside that file
    segment: u32,
    offset_in_segment: f64,
}

fn main() -> Result<()> {
    let calibrate = std::env::args().any(|a| a == "--calibrate");

    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let session_dir: PathBuf = [OUTPUT_DIR, &format!("session_{ts}")].iter().collect();
    fs::create_dir_all(&session_dir)?;
    println!("Session dir: {}", session_dir.display());

    let filter = format!(
        "ddagrab=framerate={CAPTURE_FPS}:output_idx=0,split=2[rec][det];\
         [det]fps={DET_FPS},hwdownload,format=bgra,\
         crop=iw*{CROP_W}:ih*{CROP_H}:iw*{CROP_X}:ih*{CROP_Y},scale={DET_W}:{DET_H}[d]"
    );
    let seg_pattern = session_dir.join("seg_%04d.mp4");

    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "warning", "-filter_complex", &filter])
        // Output 1: hardware-encoded segmented recording
        .args(["-map", "[rec]", "-c:v", "h264_amf", "-quality", "quality"])
        .args(["-rc", "cqp", "-qp_i", "20", "-qp_p", "22"])
        .args(["-g", &CAPTURE_FPS.to_string()]) // 1s keyframes -> accurate stream-copy clips
        .args(["-f", "segment", "-segment_time", &SEGMENT_SECS.to_string()])
        .args(["-reset_timestamps", "1", "-segment_format", "mp4"])
        .arg(seg_pattern.to_str().unwrap())
        // Output 2: raw kill-feed frames to stdout
        .args(["-map", "[d]", "-f", "rawvideo", "-pix_fmt", "bgra", "pipe:1"])
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to start ffmpeg (is it on PATH?)")?;

    let mut stdout = child.stdout.take().unwrap();
    let frame_bytes = DET_W * DET_H * 4;
    let mut buf = vec![0u8; frame_bytes];

    let mut events: Vec<Event> = Vec::new();
    let mut frame_idx: u64 = 0;
    let mut last_event_t = f64::NEG_INFINITY;
    let mut was_highlighted = false;

    println!("Recording... Ctrl+C to stop. {}", if calibrate { "(calibration mode)" } else { "" });

    while stdout.read_exact(&mut buf).is_ok() {
        let t = frame_idx as f64 / DET_FPS as f64;

        if calibrate {
            if frame_idx % DET_FPS as u64 == 0 {
                dump_ppm(&session_dir.join(format!("calib_{:05}.ppm", frame_idx)), &buf)?;
            }
        } else {
            let highlighted = count_matches(&buf) >= MIN_MATCH_PIXELS;
            // Rising edge + cooldown so one kill = one event
            if highlighted && !was_highlighted && t - last_event_t > COOLDOWN_SECS {
                let segment = (t as u32) / SEGMENT_SECS;
                let ev = Event {
                    kind: "kill",
                    t,
                    segment,
                    offset_in_segment: t - (segment * SEGMENT_SECS) as f64,
                };
                println!("KILL @ {:.1}s (seg {} +{:.1}s)", ev.t, ev.segment, ev.offset_in_segment);
                events.push(ev);
                last_event_t = t;
                // Rewrite sidecar every event so a crash loses nothing
                File::create(session_dir.join("events.json"))?
                    .write_all(serde_json::to_string_pretty(&events)?.as_bytes())?;
            }
            was_highlighted = highlighted;
        }
        frame_idx += 1;
    }

    child.wait()?;
    println!("Done. {} events.", events.len());
    Ok(())
}

/// Count pixels matching the highlight color. Frame is BGRA.
fn count_matches(frame: &[u8]) -> usize {
    frame
        .chunks_exact(4)
        .filter(|p| p[2] >= R_MIN && p[1] <= G_MAX && p[0] <= B_MAX)
        .count()
}

/// Dump a BGRA frame as a PPM (viewable in IrfanView, GIMP, etc).
fn dump_ppm(path: &PathBuf, bgra: &[u8]) -> Result<()> {
    let mut f = File::create(path)?;
    write!(f, "P6\n{DET_W} {DET_H}\n255\n")?;
    let mut rgb = Vec::with_capacity(DET_W * DET_H * 3);
    for p in bgra.chunks_exact(4) {
        rgb.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    f.write_all(&rgb)?;
    Ok(())
}
