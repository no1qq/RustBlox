use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const TAIL_BYTES: u64 = 512 * 1024;
const JOIN: &str = "! Joining game '";
const LOAD_TIME: &str = "Report game_join_loadtime:";
const DISCONNECT: &str = "Client:Disconnect";
const TELEPORT_GRACE: f64 = 3.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    pub place_id: Option<u64>,
    pub universe_id: Option<u64>,
    pub job_id: Option<String>,
    pub in_game: bool,
}

impl Activity {
    pub fn summary(&self) -> String {
        match (self.in_game, self.place_id) {
            (true, Some(place)) => format!("In place {place}"),
            (true, None) => "In a game".to_owned(),
            (false, _) => "Not in a game".to_owned(),
        }
    }

    pub fn is_same_place(&self, other: &Activity) -> bool {
        self.in_game == other.in_game && self.place_id == other.place_id
    }
}

pub fn read(log_dir: &Path) -> Activity {
    let Some(log) = newest_log(log_dir) else {
        return Activity::default();
    };
    parse(&tail_of(&log))
}

fn newest_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("log"))
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn tail_of(path: &Path) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(length) = file.metadata().map(|meta| meta.len()) else {
        return String::new();
    };

    let from = length.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }

    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn elapsed_of(line: &str) -> Option<f64> {
    line.split(',').nth(1)?.trim().parse().ok()
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.split(marker).nth(1)
}

fn field(line: &str, name: &str) -> Option<u64> {
    after(line, name)?
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

pub fn parse(log: &str) -> Activity {
    let mut activity = Activity::default();
    let mut joined_at: Option<f64> = None;
    let mut left_at: Option<f64> = None;

    for line in log.lines() {
        if line.contains(JOIN) {
            joined_at = elapsed_of(line).or(joined_at);
            activity.job_id = after(line, JOIN)
                .and_then(|rest| rest.split('\'').next())
                .filter(|id| !id.is_empty())
                .map(str::to_owned);
            activity.place_id = after(line, "' place ")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse().ok());
        } else if line.contains(LOAD_TIME) {
            if let Some(place) = field(line, "placeid:") {
                activity.place_id = Some(place);
            }
            activity.universe_id = field(line, "universeid:");
        } else if line.contains(DISCONNECT) {
            left_at = elapsed_of(line).or(left_at);
        }
    }

    activity.in_game = match (joined_at, left_at) {
        (Some(_), None) => true,
        (Some(joined), Some(left)) => left <= joined + TELEPORT_GRACE,
        (None, _) => false,
    };

    if !activity.in_game {
        activity.job_id = None;
    }

    activity
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOINED: &str = concat!(
        "2026-08-21T13:11:29.855Z,218.855469,224c,6 [FLog::Output] ! Joining game ",
        "'166f4b7b-4e07-47e3-8e38-ceeacd0246be' place 14705961406 at 10.206.17.31\n",
        "2026-08-21T13:11:29.855Z,218.855469,224c,6 [FLog::GameJoinLoadTime] Report ",
        "game_join_loadtime: placeid:14705961406, join_time:0.53, universeid:5069824722, ",
        "referral_page:, sid:1b70d5c0, clienttime:1787317890.45, userid:7119498135, \n",
    );

    const LEFT: &str = "2026-08-21T13:19:00.880Z,669.880920,224c,6,Info [DFLog::NetworkClient] Client:Disconnect\n";

    #[test]
    fn a_join_is_read_out_of_the_log() {
        let activity = parse(JOINED);

        assert!(activity.in_game);
        assert_eq!(activity.place_id, Some(14705961406));
        assert_eq!(activity.universe_id, Some(5069824722));
        assert_eq!(
            activity.job_id.as_deref(),
            Some("166f4b7b-4e07-47e3-8e38-ceeacd0246be")
        );
        assert_eq!(activity.summary(), "In place 14705961406");
    }

    #[test]
    fn a_disconnect_afterwards_means_the_game_is_over() {
        let activity = parse(&format!("{JOINED}{LEFT}"));

        assert!(!activity.in_game);
        assert_eq!(activity.summary(), "Not in a game");
        assert!(activity.job_id.is_none());
    }

    #[test]
    fn a_teleport_does_not_read_as_leaving() {
        let log = concat!(
            "2026-08-21T13:08:15.306Z,24.306892,0678,6 [FLog::Output] ! Joining game ",
            "'d259488e' place 5279837498 at 10.35.1.162\n",
            "2026-08-21T13:08:15.320Z,24.320784,224c,6,Info [DFLog::NetworkClient] Client:Disconnect\n",
        );

        let activity = parse(log);

        assert!(activity.in_game, "the old server hanging up is not a leave");
        assert_eq!(activity.place_id, Some(5279837498));
    }

    #[test]
    fn the_newest_join_is_the_one_that_counts() {
        let log = concat!(
            "2026-08-21T13:08:08.111Z,17.111080,224c,6 [FLog::Output] ! Joining game ",
            "'first' place 2337102976 at 10.30.0.140\n",
            "2026-08-21T13:11:29.855Z,218.855469,224c,6 [FLog::Output] ! Joining game ",
            "'second' place 14705961406 at 10.206.17.31\n",
        );

        let activity = parse(log);

        assert_eq!(activity.place_id, Some(14705961406));
        assert_eq!(activity.job_id.as_deref(), Some("second"));
    }

    #[test]
    fn a_log_with_no_join_is_no_activity() {
        assert_eq!(parse(""), Activity::default());
        assert_eq!(parse(LEFT), Activity::default());
        assert!(!parse("nonsense\n").in_game);
    }

    #[test]
    fn a_half_line_at_the_front_is_ignored() {
        let cut = format!("ce 12345 at 10.0.0.1\n{JOINED}");
        let activity = parse(&cut);
        assert_eq!(activity.place_id, Some(14705961406));
    }

    #[test]
    fn two_activities_compare_on_the_place_alone() {
        let one = parse(JOINED);
        let mut two = one.clone();
        two.job_id = Some("a different server".into());

        assert!(one.is_same_place(&two));

        two.place_id = Some(1);
        assert!(!one.is_same_place(&two));
    }

    #[test]
    fn a_missing_folder_reads_as_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("nope")), Activity::default());
    }
}
