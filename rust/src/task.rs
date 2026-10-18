//! The job queue: upscaling, GIF split/merge, lossy compression and custom
//! post-processing commands. Tasks run on a worker thread and report back to
//! the UI through a channel.

use std::error::Error;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam_channel::Sender;

use crate::image_util::{self, Filter};
use crate::params::{ReConfigParams, ResizeMode};

pub type TaskResult<T = ()> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub enum UiMsg {
    Log(String),
    /// Overall progress in percent.
    Progress(f64),
    /// A run finished; `with_error` is true when some task failed.
    Done { with_error: bool, elapsed: f64 },
    /// A single task failed hard (processing stopped).
    Failed(String),
    /// The worker thread is done and the app should return to the idle state.
    Finished,
}

#[derive(Debug, Default)]
pub struct Progress {
    pub fraction: f64,
    pub done: usize,
    pub total: usize,
}

/// Everything a task needs: parameters, channels and shared state.
pub struct TaskCtx {
    pub params: ReConfigParams,
    pub re_path: PathBuf,
    pub realcugan: bool,
    pub upscayl: bool,
    pub optimize_gif: bool,
    pub custom_command: String,
    pub tx: Sender<UiMsg>,
    pub progress: Mutex<Progress>,
    /// `true` while running, `false` while paused.
    pub pause: AtomicBool,
    /// `false` requests cancellation.
    pub running: AtomicBool,
    pub temp_dir: PathBuf,
    counter: AtomicUsize,
}

impl TaskCtx {
    pub fn new(
        params: ReConfigParams,
        re_path: PathBuf,
        realcugan: bool,
        upscayl: bool,
        optimize_gif: bool,
        custom_command: String,
        tx: Sender<UiMsg>,
        temp_dir: PathBuf,
    ) -> Self {
        Self {
            params,
            re_path,
            realcugan,
            upscayl,
            optimize_gif,
            custom_command,
            tx,
            progress: Mutex::new(Progress::default()),
            pause: AtomicBool::new(true),
            running: AtomicBool::new(true),
            temp_dir,
            counter: AtomicUsize::new(0),
        }
    }

    pub fn log(&self, s: impl Into<String>) {
        let _ = self.tx.send(UiMsg::Log(s.into()));
    }

    fn emit_progress(&self) {
        let (percent, _) = {
            let p = self.progress.lock().unwrap();
            let total = p.total.max(1) as f64;
            (((p.fraction + p.done as f64) / total * 100.0).clamp(0.0, 100.0), ())
        };
        let _ = self.tx.send(UiMsg::Progress(percent));
    }

    pub fn set_fraction(&self, f: f64) {
        self.progress.lock().unwrap().fraction = f;
        self.emit_progress();
    }

    pub fn inc_done(&self) {
        {
            let mut p = self.progress.lock().unwrap();
            p.done += 1;
            p.fraction = 0.0;
        }
        self.emit_progress();
    }

    pub fn add_total(&self) {
        self.progress.lock().unwrap().total += 1;
        self.emit_progress();
    }

    pub fn sub_total(&self) {
        let mut p = self.progress.lock().unwrap();
        p.total = p.total.saturating_sub(1);
        drop(p);
        self.emit_progress();
    }

    pub fn set_total(&self, total: usize) {
        self.progress.lock().unwrap().total = total;
    }

    pub fn temp_path(&self, ext: &str) -> PathBuf {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let ext = ext.trim_start_matches('.');
        self.temp_dir.join(format!("{n:06}.{ext}"))
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Block while paused, abort when cancellation was requested.
    pub fn wait_pause(&self) -> TaskResult {
        while !self.pause.load(Ordering::Relaxed) {
            if !self.running() {
                return Err("cancelled".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !self.running() {
            return Err("cancelled".into());
        }
        Ok(())
    }
}

pub trait Task: Send {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult;
}

fn gpu_arg(gpu_id: i64) -> String {
    if gpu_id < 0 {
        "auto".to_string()
    } else {
        gpu_id.to_string()
    }
}

/// Map a Real-CUGAN model suffix to its denoise level argument.
fn realcugan_denoise(model_filename: &str) -> i64 {
    match model_filename.split_once('-').map(|(_, r)| r).unwrap_or("") {
        "conservative" => -1,
        "no-denoise" => 0,
        "denoise1x" => 1,
        "denoise2x" => 2,
        "denoise3x" => 3,
        _ => 0,
    }
}

fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

fn build_command(ctx: &TaskCtx, input: &Path, output: &Path, factor: i64) -> Command {
    let p = &ctx.params;
    let mut c = Command::new(&ctx.re_path);
    c.arg("-v")
        .arg("-i")
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("-s")
        .arg(factor.to_string());

    if ctx.realcugan {
        let (model_name, model_filename) = p.model.split_once('#').unwrap_or((p.model.as_str(), ""));
        let model_path = p.model_dir.join(model_name);
        c.arg("-t")
            .arg(p.tile_size.to_string())
            .arg("-m")
            .arg(model_path)
            .arg("-n")
            .arg(realcugan_denoise(model_filename).to_string())
            .arg("-g")
            .arg(gpu_arg(p.gpu_id))
            .arg("-c")
            .arg("1");
    } else {
        if ctx.upscayl {
            c.arg("-z").arg(factor.to_string());
        }
        c.arg("-t")
            .arg(p.tile_size.to_string())
            .arg("-n")
            .arg(&p.model)
            .arg("-g")
            .arg(gpu_arg(p.gpu_id));
    }

    if p.use_tta {
        c.arg("-x");
    }

    c.stdout(Stdio::null()).stderr(Stdio::piped());
    hide_window(&mut c);
    c
}

fn parse_alpha_override(line: &str) -> Option<String> {
    if line.starts_with("image ") && line.contains("has alpha channel !") {
        line.find("will output ")
            .map(|i| line[i + "will output ".len()..].trim().to_string())
    } else {
        None
    }
}

fn parse_percent(line: &str) -> Option<f64> {
    let pos = line.find('%')?;
    let head = &line[..pos];
    let mut num = String::new();
    for ch in head.chars().rev() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
            num.insert(0, ch);
        } else {
            break;
        }
    }
    if !num.contains('.') && !num.contains(',') {
        return None;
    }
    num.replace(',', ".").parse::<f64>().ok().map(|v| v / 100.0)
}

fn is_done_line(line: &str) -> bool {
    let t = line.trim_end();
    t.ends_with("done") && t.contains(" -> ")
}

/// Run an external command, streaming its stderr to the log.
fn run_external(ctx: &TaskCtx, mut cmd: Command, mut on_line: impl FnMut(&str)) -> TaskResult {
    let mut child = cmd.spawn()?;
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            ctx.log(format!("{line}\n"));
            on_line(&line);
            if !ctx.running() {
                let _ = child.kill();
            }
        }
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("process exited with status {status}").into());
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> TaskResult {
    if from == to {
        return Ok(());
    }
    if to.exists() {
        let _ = std::fs::remove_file(to);
    }
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to)?;
        let _ = std::fs::remove_file(from);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Upscaling
// ---------------------------------------------------------------------------

pub struct ReSpawnTask {
    pub input: PathBuf,
    pub output: PathBuf,
    pub remove_input: bool,
}

impl ReSpawnTask {
    pub fn new(input: PathBuf, output: PathBuf, remove_input: bool) -> Self {
        Self { input, output, remove_input }
    }

    /// Resolve `LongestSide` / `ShortestSide` into a concrete dimension.
    fn effective_mode(mode: ResizeMode, w: u32, h: u32) -> ResizeMode {
        match mode {
            ResizeMode::LongestSide => {
                if w >= h {
                    ResizeMode::Width
                } else {
                    ResizeMode::Height
                }
            }
            ResizeMode::ShortestSide => {
                if w <= h {
                    ResizeMode::Width
                } else {
                    ResizeMode::Height
                }
            }
            other => other,
        }
    }

    fn target_size(mode: ResizeMode, value: i64, src_w: u32, src_h: u32) -> (u32, u32) {
        let ratio = src_w as f64 / src_h.max(1) as f64;
        match mode {
            ResizeMode::Ratio => (
                (src_w as f64 * value as f64).round() as u32,
                (src_h as f64 * value as f64).round() as u32,
            ),
            ResizeMode::Width => {
                let w = value as f64;
                (w.round() as u32, (w / ratio).round() as u32)
            }
            ResizeMode::Height => {
                let h = value as f64;
                ((h * ratio).round() as u32, h.round() as u32)
            }
            _ => (src_w, src_h),
        }
    }
}

impl Task for ReSpawnTask {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult {
        let cfg = &ctx.params;
        ctx.log(format!("Using executable: {}\n", ctx.re_path.display()));
        ctx.set_fraction(0.0);

        let (src_w0, src_h0) = image_util::dimensions(&self.input)?;
        let mode = Self::effective_mode(cfg.resize_mode, src_w0, src_h0);
        let (dst_w, dst_h) = Self::target_size(mode, cfg.resize_mode_value, src_w0, src_h0);

        let mut src_w = src_w0;
        let mut src_h = src_h0;
        let mut work_input = self.input.clone();
        let mut preupscale_temp: Option<PathBuf> = None;

        // Optional pre-upscale with a regular resampling filter.
        if cfg.preupscale && cfg.model_factor > 0 && dst_w > 0 && dst_h > 0 {
            let scale_ratio = match mode {
                ResizeMode::Ratio => cfg.resize_mode_value as f64,
                ResizeMode::Width => cfg.resize_mode_value as f64 / src_w0 as f64,
                ResizeMode::Height => cfg.resize_mode_value as f64 / src_h0 as f64,
                _ => 1.0,
            };
            if scale_ratio > 0.0 {
                let l = scale_ratio.log(cfg.model_factor as f64);
                let intg = l.floor();
                let frac = l - intg;
                let pow = (cfg.model_factor as f64).powi(intg as i32);
                let pre_w = (dst_w as f64 / pow).ceil() as u32;
                let pre_h = (dst_h as f64 / pow).ceil() as u32;
                if frac < 0.5 && (pre_w != src_w || pre_h != src_h) && pre_w > 0 && pre_h > 0 {
                    ctx.log(format!("Pre-upscale from {src_w}x{src_h} to {pre_w}x{pre_h}.\n"));
                    let img = image_util::load(&work_input)?;
                    let resized = image_util::resize(&img, pre_w, pre_h, Filter::Lanczos3);
                    let ext = if image_util::is_webp(&work_input) { "webp" } else { "png" };
                    let tmp = ctx.temp_path(ext);
                    image_util::save_lossless(&resized, &tmp)?;
                    preupscale_temp = Some(tmp.clone());
                    work_input = tmp;
                    src_w = pre_w;
                    src_h = pre_h;
                }
            }
        }

        let factor = cfg.model_factor.max(1);
        let mut cur_w = src_w as i64;
        let mut cur_h = src_h as i64;
        let mut scale_pass: usize = 0;
        while cur_w < dst_w as i64 && cur_h < dst_h as i64 && scale_pass < 8 {
            scale_pass += 1;
            cur_w *= factor;
            cur_h *= factor;
        }

        let out_ext = self
            .output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_string();

        let mut files: Vec<PathBuf> = vec![work_input.clone()];
        for _ in 0..scale_pass {
            files.push(ctx.temp_path(&out_ext));
        }

        let mut alpha_override: Option<PathBuf> = None;
        let n = files.len();
        if n > 1 {
            let denom = (n - 1) as f64;
            for i in 0..n - 1 {
                if !ctx.running() {
                    return Err("cancelled".into());
                }
                ctx.wait_pause()?;
                let cmd = build_command(ctx, &files[i], &files[i + 1], factor);
                let idx = i as f64;
                let alpha = &mut alpha_override;
                run_external(ctx, cmd, |line| {
                    if let Some(p) = parse_alpha_override(line) {
                        *alpha = Some(PathBuf::from(p));
                    } else if let Some(f) = parse_percent(line) {
                        ctx.set_fraction((idx + f) / denom);
                    } else if is_done_line(line) {
                        ctx.set_fraction((idx + 1.0) / denom);
                    }
                })?;
            }
        }

        if let Some(parent) = self.output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        if let Some(ov) = &alpha_override {
            ctx.log(format!("Rename {} to {}.\n", ov.display(), self.output.display()));
            copy_file(ov, &self.output)?;
        } else if src_w == dst_w && src_h == dst_h {
            copy_file(&files[n - 1], &self.output)?;
        } else {
            ctx.log(format!("Downsample from {src_w}x{src_h} to {dst_w}x{dst_h}.\n"));
            let img = image_util::load(&files[n - 1])?;
            let resized = image_util::resize(&img, dst_w, dst_h, cfg.downsample.filter());
            image_util::save_lossless(&resized, &self.output)?;
        }

        // Clean up temporary files. The user's original input file is never removed here.
        if let Some(t) = &preupscale_temp {
            let _ = std::fs::remove_file(t);
        }
        for f in removable_temps(&files, &self.input) {
            let _ = std::fs::remove_file(f);
        }
        if self.remove_input && work_input != self.input {
            let _ = std::fs::remove_file(&work_input);
        }
        if self.remove_input {
            let _ = std::fs::remove_file(&self.input);
        }

        if ctx.running() {
            ctx.inc_done();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Lossy compression
// ---------------------------------------------------------------------------

pub struct LossyCompressTask {
    pub input: PathBuf,
    pub output: PathBuf,
    pub quality: i64,
    pub remove_input: bool,
}

impl Task for LossyCompressTask {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult {
        ctx.log(format!(
            "Compressing {} to {} with quality {}.\n",
            self.input.display(),
            self.output.display(),
            self.quality
        ));
        let img = image_util::load(&self.input)?;
        image_util::save_lossy(&img, &self.output, self.quality, || {
            ctx.log("Discarding alpha channel to compress the RGBA image to JPEG.\n");
        })?;
        if self.remove_input {
            let _ = std::fs::remove_file(&self.input);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Custom post-processing command
// ---------------------------------------------------------------------------

pub struct CustomCompressTask {
    pub input: PathBuf,
    pub output: PathBuf,
    pub template: String,
    pub remove_input: bool,
}

impl Task for CustomCompressTask {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult {
        let tokens = shlex::split(&self.template)
            .ok_or_else(|| -> Box<dyn Error + Send + Sync> { "invalid command template".into() })?;
        let mut args: Vec<String> = Vec::with_capacity(tokens.len());
        for token in tokens {
            if token == "{input}" {
                args.push(self.input.to_string_lossy().to_string());
            } else if token == "{output}" {
                args.push(self.output.to_string_lossy().to_string());
            } else if let Some(rest) = token.strip_prefix("{output:") {
                if let Some(ext) = rest.strip_suffix('}') {
                    let dir = self.output.parent().map(Path::to_path_buf).unwrap_or_default();
                    let stem = self
                        .output
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    args.push(dir.join(format!("{stem}.{ext}")).to_string_lossy().to_string());
                } else {
                    args.push(token);
                }
            } else {
                args.push(token);
            }
        }
        if args.is_empty() {
            return Err("empty command".into());
        }
        ctx.log(format!(
            "Compressing {} with command: {}.\n",
            self.input.display(),
            args.join(" ")
        ));
        if let Some(parent) = self.output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        cmd.stdout(Stdio::null()).stderr(Stdio::piped());
        hide_window(&mut cmd);
        run_external(ctx, cmd, |_| {})?;
        if self.remove_input {
            let _ = std::fs::remove_file(&self.input);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GIF split / merge
// ---------------------------------------------------------------------------

/// Extract every frame of a GIF, upscale them and merge the result back into a
/// GIF. Runs entirely inline on the worker thread.
pub struct GifSplitTask {
    pub input: PathBuf,
    pub output: PathBuf,
}

impl Task for GifSplitTask {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult {
        let file = std::fs::File::open(&self.input)?;
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(file)?;
        let screen_w = decoder.width().max(1);
        let screen_h = decoder.height().max(1);

        let mut canvas = image::RgbaImage::new(screen_w as u32, screen_h as u32);
        let mut frames: Vec<PathBuf> = Vec::new();
        let mut delays: Vec<u16> = Vec::new();
        let mut index = 0usize;

        while let Some(frame) = decoder.read_next_frame()? {
            let previous = if matches!(frame.dispose, gif::DisposalMethod::Previous) {
                Some(canvas.clone())
            } else {
                None
            };

            // Composite this frame onto the canvas at its offset.
            let fw = frame.width as u32;
            let fh = frame.height as u32;
            if let Some(src) = image::RgbaImage::from_raw(fw, fh, frame.buffer.to_vec()) {
                let (ox, oy) = (frame.left as i64, frame.top as i64);
                for y in 0..fh as i64 {
                    for x in 0..fw as i64 {
                        let dx = ox + x;
                        let dy = oy + y;
                        if dx < 0 || dy < 0 || dx >= screen_w as i64 || dy >= screen_h as i64 {
                            continue;
                        }
                        let px = src.get_pixel(x as u32, y as u32);
                        if px[3] == 0 {
                            continue;
                        }
                        canvas.put_pixel(dx as u32, dy as u32, *px);
                    }
                }
            }

            let mut out = canvas.clone();
            if ctx.optimize_gif {
                composite_over_white(&mut out);
            }
            let frame_src = ctx.temp_path("png");
            let frame_dst = ctx.temp_path("png");
            image_util::save_lossless(&image::DynamicImage::ImageRgba8(out), &frame_src)?;

            ctx.log(format!(
                "Frame #{index}: {} -> {} Duration: {}\n",
                frame_src.display(),
                frame_dst.display(),
                frame.delay
            ));

            ctx.add_total();
            let mut sub = ReSpawnTask::new(frame_src, frame_dst.clone(), true);
            sub.run(ctx)?;

            frames.push(frame_dst);
            delays.push(frame.delay.max(1));
            index += 1;

            match frame.dispose {
                gif::DisposalMethod::Background => {
                    let (ox, oy) = (frame.left as u32, frame.top as u32);
                    for y in oy..(oy + fh).min(screen_h as u32) {
                        for x in ox..(ox + fw).min(screen_w as u32) {
                            canvas.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                        }
                    }
                }
                gif::DisposalMethod::Previous => {
                    if let Some(prev) = previous {
                        canvas = prev;
                    }
                }
                _ => {}
            }
        }

        // The split task itself was counted by the caller; the merge is not a
        // separate progress unit in the queue accounting.
        ctx.sub_total();

        let merge_target = if ctx.custom_command.trim().is_empty() {
            self.output.clone()
        } else {
            ctx.temp_path("gif")
        };

        GifMergeTask {
            output: merge_target.clone(),
            frames,
            delays,
        }
        .run(ctx)?;

        if !ctx.custom_command.trim().is_empty() {
            let mut custom = CustomCompressTask {
                input: merge_target,
                output: self.output.clone(),
                template: ctx.custom_command.clone(),
                remove_input: true,
            };
            custom.run(ctx)?;
        }
        Ok(())
    }
}

fn composite_over_white(img: &mut image::RgbaImage) {
    for px in img.pixels_mut() {
        if px[3] == 0 {
            *px = image::Rgba([255, 255, 255, 255]);
        }
    }
}

pub struct GifMergeTask {
    pub output: PathBuf,
    pub frames: Vec<PathBuf>,
    pub delays: Vec<u16>,
}

impl Task for GifMergeTask {
    fn run(&mut self, ctx: &TaskCtx) -> TaskResult {
        ctx.log(format!(
            "Merging {} frames to {}.\n",
            self.frames.len(),
            self.output.display()
        ));
        if let Some(parent) = self.output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut decoded: Vec<image::RgbaImage> = Vec::with_capacity(self.frames.len());
        let (mut width, mut height) = (0u16, 0u16);
        for path in &self.frames {
            let img = image_util::load(path)?.to_rgba8();
            width = width.max(img.width().min(u16::MAX as u32) as u16);
            height = height.max(img.height().min(u16::MAX as u32) as u16);
            decoded.push(img);
        }
        width = width.max(1);
        height = height.max(1);

        let file = std::fs::File::create(&self.output)?;
        let mut encoder = gif::Encoder::new(file, width, height, &[])?;
        encoder.set_repeat(gif::Repeat::Infinite)?;

        for (i, img) in decoded.iter().enumerate() {
            let mut canvas = image::RgbaImage::new(width as u32, height as u32);
            image::imageops::replace(&mut canvas, img, 0, 0);
            let mut pixels = canvas.into_raw();
            let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
            frame.delay = self.delays.get(i).copied().unwrap_or(5).max(1);
            frame.dispose = gif::DisposalMethod::Background;
            encoder.write_frame(&frame)?;
        }

        for f in &self.frames {
            let _ = std::fs::remove_file(f);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

/// Run `tasks` on the current thread, reporting to `ctx`.
pub fn run_queue(mut tasks: Vec<Box<dyn Task>>, ctx: Arc<TaskCtx>, ignore_error: bool) {
    let started = Instant::now();
    let mut with_error = false;
    let mut counter = 0usize;
    for task in tasks.iter_mut() {
        if !ctx.running() {
            break;
        }
        if ctx.wait_pause().is_err() {
            break;
        }
        let ts = Instant::now();
        match task.run(&ctx) {
            Ok(()) => {
                ctx.log(format!(
                    "Task #{counter} completed in {}ms.\n",
                    ts.elapsed().as_millis()
                ));
                counter += 1;
            }
            Err(e) => {
                if e.to_string() == "cancelled" {
                    break;
                }
                with_error = true;
                ctx.log(format!("{e}\n"));
                let _ = ctx.tx.send(UiMsg::Failed(e.to_string()));
                if !ignore_error {
                    let _ = ctx.tx.send(UiMsg::Finished);
                    return;
                }
            }
        }
    }
    let _ = ctx.tx.send(UiMsg::Done {
        with_error,
        elapsed: started.elapsed().as_secs_f64(),
    });
    let _ = ctx.tx.send(UiMsg::Finished);
}

/// Paths that may be deleted after a run: every temporary file, never the input file.
fn removable_temps(files: &[PathBuf], input: &Path) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|f| f.as_path() != input)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::removable_temps;
    use std::path::PathBuf;

    #[test]
    fn input_file_is_never_listed_for_deletion() {
        let input = PathBuf::from(r"C:\images\photo.png");
        let files = vec![
            input.clone(),
            PathBuf::from(r"C:\temp\pass1.png"),
            PathBuf::from(r"C:\temp\pass2.png"),
        ];
        let temps = removable_temps(&files, &input);
        assert!(!temps.contains(&input));
        assert_eq!(
            temps,
            vec![
                PathBuf::from(r"C:\temp\pass1.png"),
                PathBuf::from(r"C:\temp\pass2.png")
            ]
        );
    }

    #[test]
    fn preupscaled_temp_is_removed_but_source_is_kept() {
        let input = PathBuf::from(r"C:\images\photo.png");
        let pre = PathBuf::from(r"C:\temp\pre.png");
        let files = vec![pre.clone(), PathBuf::from(r"C:\temp\pass1.png")];
        let temps = removable_temps(&files, &input);
        assert!(temps.contains(&pre));
        assert!(!temps.contains(&input));
    }
}