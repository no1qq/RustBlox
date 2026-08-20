use chrono::{DateTime, Local};

pub fn relative_time(then: DateTime<Local>) -> String {
    let delta = Local::now().signed_duration_since(then);
    let seconds = delta.num_seconds();

    if seconds < 0 {
        return "just now".into();
    }
    if seconds < 45 {
        return "just now".into();
    }
    if seconds < 90 {
        return "a minute ago".into();
    }

    let minutes = delta.num_minutes();
    if minutes < 60 {
        return format!("{minutes} minutes ago");
    }

    let hours = delta.num_hours();
    if hours < 24 {
        return plural(hours, "hour");
    }

    let days = delta.num_days();
    if days < 7 {
        return plural(days, "day");
    }
    if days < 30 {
        return plural(days / 7, "week");
    }
    if days < 365 {
        return plural(days / 30, "month");
    }
    plural(days / 365, "year")
}

fn plural(value: i64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{value} {unit}s ago")
    }
}

pub fn timestamp(then: DateTime<Local>) -> String {
    then.format("%d %b %Y at %H:%M").to_string()
}

pub fn elapsed(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    if total < 60 {
        return format!("{total}s");
    }
    let minutes = total / 60;
    if minutes < 60 {
        return format!("{}m {}s", minutes, total % 60);
    }
    format!("{}h {}m", minutes / 60, minutes % 60)
}

pub fn truncate_middle(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars < 8 {
        return text.to_owned();
    }
    let keep = max_chars - 3;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push_str("...");
    out.extend(chars[chars.len() - tail..].iter());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn ago(duration: Duration) -> DateTime<Local> {
        Local::now() - duration
    }

    #[test]
    fn describes_recent_moments_loosely() {
        assert_eq!(relative_time(ago(Duration::seconds(5))), "just now");
        assert_eq!(relative_time(ago(Duration::seconds(60))), "a minute ago");
        assert_eq!(relative_time(ago(Duration::minutes(5))), "5 minutes ago");
        assert_eq!(relative_time(ago(Duration::hours(1))), "1 hour ago");
        assert_eq!(relative_time(ago(Duration::hours(5))), "5 hours ago");
        assert_eq!(relative_time(ago(Duration::days(2))), "2 days ago");
        assert_eq!(relative_time(ago(Duration::days(14))), "2 weeks ago");
        assert_eq!(relative_time(ago(Duration::days(400))), "1 year ago");
    }

    #[test]
    fn future_timestamps_do_not_panic() {
        assert_eq!(relative_time(Local::now() + Duration::hours(1)), "just now");
    }

    #[test]
    fn formats_elapsed_durations() {
        assert_eq!(elapsed(std::time::Duration::from_secs(9)), "9s");
        assert_eq!(elapsed(std::time::Duration::from_secs(75)), "1m 15s");
        assert_eq!(elapsed(std::time::Duration::from_secs(3700)), "1h 1m");
    }

    #[test]
    fn truncates_long_paths_in_the_middle() {
        assert_eq!(truncate_middle("short", 20), "short");
        assert_eq!(truncate_middle("abcdefghijklmnop", 10), "abc...mnop");
        assert_eq!(truncate_middle("abcdefghijklmnop", 10).chars().count(), 10);
        assert_eq!(truncate_middle("abcdefghij", 4), "abcdefghij");
    }
}
