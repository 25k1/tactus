use crate::tags::Tags;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use tactus_api::error::{Error, Result};
use tactus_api::net;
use tactus_api::{Quality, Service, Track};

pub const TEMPLATES: [&str; 4] =
    ["{artist}/{album}/{number} {title}", "{artist} - {title}", "{album}/{number} {title}", "{artist}/{title}"];

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadState {
    Queued,
    Downloading(u8),
    Finished(PathBuf),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub id: u64,
    pub track: Track,
    pub state: DownloadState,
}

impl Download {
    pub fn file(&self) -> Option<&Path> {
        match &self.state {
            DownloadState::Finished(path) => Some(path),
            _ => None,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, DownloadState::Finished(_) | DownloadState::Failed(_))
    }
}

pub struct Job {
    pub track: Track,
    pub path: PathBuf,
    pub number: Option<usize>,
    pub quality: Quality,
    pub cover: bool,
    pub service: Service,
}

pub struct Update {
    pub id: u64,
    pub state: DownloadState,
}

pub struct Downloader {
    jobs: Sender<(u64, Job)>,
    updates: Receiver<Update>,
    next_id: u64,
}

impl Default for Downloader {
    fn default() -> Self {
        let (jobs, queue) = mpsc::channel::<(u64, Job)>();
        let (sender, updates) = mpsc::channel();
        std::thread::spawn(move || {
            for (id, job) in queue {
                let state = match run(id, &job, &sender) {
                    Ok(path) => DownloadState::Finished(path),
                    Err(error) => DownloadState::Failed(error.to_string()),
                };
                let _ = sender.send(Update { id, state });
            }
        });
        Self { jobs, updates, next_id: 1 }
    }
}

impl Downloader {
    pub fn enqueue(&mut self, job: Job) -> Download {
        let id = self.next_id;
        self.next_id += 1;
        let track = job.track.clone();
        let _ = self.jobs.send((id, job));
        Download { id, track, state: DownloadState::Queued }
    }

    pub fn updates(&self) -> impl Iterator<Item = Update> + '_ {
        self.updates.try_iter()
    }
}

pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim();
    cleaned.chars().take(150).collect()
}

pub fn render_path(folder: &Path, template: &str, track: &Track, number: Option<usize>) -> PathBuf {
    let year = if track.year > 0 { track.year.to_string() } else { String::new() };
    let number = number.map(|number| format!("{number:02}")).unwrap_or_default();
    let fields = [
        ("{artist}", track.artist.as_str()),
        ("{album}", track.album.as_str()),
        ("{title}", &track.full_title()),
        ("{number}", &number),
        ("{year}", &year),
        ("{genre}", track.genre.as_str()),
    ];
    let mut segments: Vec<String> = template
        .split(['/', '\\'])
        .map(|segment| {
            let rendered = fields.iter().fold(segment.to_owned(), |text, (key, value)| text.replace(key, value));
            safe_file_name(rendered.trim_matches(|c: char| c == '-' || c.is_whitespace()))
        })
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() || template.trim_end().ends_with(['/', '\\']) {
        segments.push(safe_file_name(&track.full_title()));
    }
    if segments.last().is_none_or(String::is_empty) {
        segments.push("track".to_owned());
    }
    let mut path = folder.to_path_buf();
    path.extend(segments.iter().filter(|segment| !segment.is_empty()));
    path
}

fn run(id: u64, job: &Job, updates: &Sender<Update>) -> Result<PathBuf> {
    let report = |percent: u8| {
        let _ = updates.send(Update { id, state: DownloadState::Downloading(percent) });
    };
    report(0);
    let source = job.service.audio_source(&job.track.id, job.quality)?;
    let path = job.path.with_extension(source.codec.extension());
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let partial = path.with_extension(format!("{}.part", source.codec.extension()));
    net::block_on(async {
        let mut response = net::client().get(&source.url).send().await?.error_for_status()?;
        let total = response.content_length().unwrap_or(0);
        let mut file = std::fs::File::create(&partial)?;
        let (mut written, mut reported) = (0u64, 0u8);
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
            written += chunk.len() as u64;
            let percent = (written * 100).checked_div(total).unwrap_or(0).min(99) as u8;
            if percent >= reported + 5 {
                reported = percent;
                report(percent);
            }
        }
        file.flush()?;
        Ok::<_, Error>(())
    })?;
    let cover = if job.cover && !job.track.cover.is_empty() {
        net::block_on(async { net::client().get(&job.track.cover).send().await.ok()?.bytes().await.ok() })
    } else {
        None
    };
    if let Err(error) = Tags::new(&job.track, job.number, cover.as_deref()).write(&partial) {
        crate::log::warn(format_args!("tagging {}: {error}", path.display()));
    }
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::fake::track;

    #[test]
    fn makes_names_safe_for_windows() {
        assert_eq!(safe_file_name("AC/DC - Back: In Black?"), "AC_DC - Back_ In Black_");
        assert_eq!(safe_file_name("  trailing dots... "), "trailing dots");
        assert_eq!(safe_file_name(""), "");
    }

    #[test]
    fn renders_templates() {
        let folder = Path::new("music");
        let track = track("1");
        assert_eq!(
            render_path(folder, "{artist}/{album}/{number} {title}", &track, Some(3)),
            folder.join("Artist").join("Album").join("03 Track 1")
        );
        assert_eq!(
            render_path(folder, "{artist}/{album}/{number} {title}", &track, None),
            folder.join("Artist").join("Album").join("Track 1")
        );
        assert_eq!(render_path(folder, "{artist} - {title}", &track, None), folder.join("Artist - Track 1"));
        assert_eq!(render_path(folder, "{album}/", &track, None), folder.join("Album").join("Track 1"));
        assert_eq!(render_path(folder, "", &track, None), folder.join("Track 1"));
    }
}
